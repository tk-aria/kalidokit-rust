//! Plugin client (host-side).
//!
//! The `Client` struct manages the full lifecycle of a plugin subprocess:
//! spawning, protocol negotiation, transport setup, dispensing interfaces,
//! and graceful/forceful shutdown.
//!
//! Mirrors Go's `client.go`.

use crate::error::PluginError;
use crate::grpc_client::GRPCClient;
use crate::grpc_stdio::GRPCStdioClient;
use crate::mtls::MtlsConfig;
use crate::protocol::{
    self, HandshakeConfig, Protocol, ServerNegotiationLine,
    ENV_PLUGIN_CLIENT_CERT, ENV_PLUGIN_MAX_PORT, ENV_PLUGIN_MIN_PORT,
    ENV_PLUGIN_PROTOCOL_VERSIONS,
};
use crate::runner::{CmdRunner, PluginAddr, ReattachConfig, Runner};
use crate::secure::SecureConfig;
use std::any::Any;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_START_TIMEOUT: Duration = Duration::from_secs(60);
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Configuration for the plugin client (host side).
pub struct ClientConfig {
    pub handshake: HandshakeConfig,
    pub plugins: HashMap<String, Box<dyn crate::grpc_client::GRPCPluginFactory>>,
    pub versioned_plugins: HashMap<u32, HashMap<String, Box<dyn crate::grpc_client::GRPCPluginFactory>>>,
    pub cmd: Option<CmdRunner>,
    pub reattach: Option<ReattachConfig>,
    pub auto_mtls: bool,
    pub secure_config: Option<SecureConfig>,
    pub allowed_protocols: Vec<Protocol>,
    pub start_timeout: Duration,
    pub managed: bool,
    pub min_port: u16,
    pub max_port: u16,
    pub stderr: Option<Box<dyn std::io::Write + Send>>,
    pub sync_stdout: Option<Box<dyn std::io::Write + Send>>,
    pub sync_stderr: Option<Box<dyn std::io::Write + Send>>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            handshake: HandshakeConfig {
                protocol_version: 1,
                magic_cookie_key: String::new(),
                magic_cookie_value: String::new(),
            },
            plugins: HashMap::new(),
            versioned_plugins: HashMap::new(),
            cmd: None,
            reattach: None,
            auto_mtls: true,
            secure_config: None,
            allowed_protocols: vec![Protocol::Grpc],
            start_timeout: DEFAULT_START_TIMEOUT,
            managed: false,
            min_port: 10000,
            max_port: 25000,
            stderr: None,
            sync_stdout: None,
            sync_stderr: None,
        }
    }
}

/// Plugin client that manages the lifecycle of a single plugin process.
pub struct Client {
    config: ClientConfig,
    /// Running process handle (child only, stderr moved out for logging).
    child: Option<std::process::Child>,
    grpc_client: Option<GRPCClient>,
    negotiated_version: Option<u32>,
    address: Option<PluginAddr>,
    exited: Arc<AtomicBool>,
    mtls: Option<MtlsConfig>,
}

impl Client {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            child: None,
            grpc_client: None,
            negotiated_version: None,
            address: None,
            exited: Arc::new(AtomicBool::new(false)),
            mtls: None,
        }
    }

    /// Start the plugin process and establish the RPC connection.
    pub async fn start(&mut self) -> Result<(), PluginError> {
        let source_count = self.config.cmd.is_some() as u8
            + self.config.reattach.is_some() as u8;
        if source_count != 1 {
            return Err(PluginError::Other(
                "exactly one of cmd or reattach must be set in ClientConfig".into(),
            ));
        }

        // Handle reattach
        if let Some(reattach) = &self.config.reattach {
            self.negotiated_version = Some(reattach.protocol_version);
            self.address = Some(reattach.addr.clone());
            return self.connect_grpc(&reattach.addr.address()).await;
        }

        let mut cmd = self.config.cmd.take().unwrap();

        // Checksum verification
        if let Some(secure) = &self.config.secure_config {
            secure.check(&cmd.cmd)?;
        }

        // Generate mTLS certs
        if self.config.auto_mtls {
            let mtls = MtlsConfig::generate()?;
            self.mtls = Some(mtls);
        }

        // Build environment variables
        cmd = cmd.env(
            &self.config.handshake.magic_cookie_key,
            &self.config.handshake.magic_cookie_value,
        );
        cmd = cmd.env(ENV_PLUGIN_MIN_PORT, self.config.min_port.to_string());
        cmd = cmd.env(ENV_PLUGIN_MAX_PORT, self.config.max_port.to_string());

        let versions = self.supported_versions();
        cmd = cmd.env(
            ENV_PLUGIN_PROTOCOL_VERSIONS,
            protocol::encode_version_list(&versions),
        );

        if let Some(mtls) = &self.mtls {
            cmd = cmd.env(ENV_PLUGIN_CLIENT_CERT, mtls.ca_cert_base64());
        }

        // Start with timeout
        let start_timeout = self.config.start_timeout;
        let mut running = tokio::time::timeout(
            start_timeout,
            tokio::task::spawn_blocking(move || cmd.start()),
        )
        .await
        .map_err(|_| PluginError::StartTimeout(start_timeout))?
        .map_err(|e| PluginError::Subprocess(format!("join error: {e}")))??;

        // Move stderr to logger thread
        if let Some(stderr) = running.stderr.take() {
            let stderr_writer = self.config.stderr.take();
            let exited = self.exited.clone();
            std::thread::spawn(move || {
                log_stderr(stderr, stderr_writer, exited);
            });
        }

        // Read negotiation line
        let neg_line_str = running.read_negotiation_line()?;
        let neg = ServerNegotiationLine::parse(&neg_line_str)?;

        neg.validate_core_version()?;

        let _version = protocol::negotiate_version(
            &self.supported_versions(),
            neg.app_protocol_version,
        )?;

        self.negotiated_version = Some(neg.app_protocol_version);
        self.address = Some(PluginAddr::parse(&neg.network, &neg.address)?);

        log::info!(
            "Plugin started (pid={}, version={}, protocol={:?}, addr={})",
            running.id(),
            neg.app_protocol_version,
            neg.protocol,
            neg.address,
        );

        self.child = Some(running.child);
        self.connect_grpc(&neg.address).await?;

        // Start stdio client to mirror plugin stdout/stderr (matching Go behavior).
        // Uses sync_stdout/sync_stderr from config, falling back to std::io::sink.
        if let Some(client) = &self.grpc_client {
            let channel = client.channel().clone();
            let stdout: Box<dyn std::io::Write + Send> = self
                .config
                .sync_stdout
                .take()
                .unwrap_or_else(|| Box::new(std::io::stdout()));
            let stderr: Box<dyn std::io::Write + Send> = self
                .config
                .sync_stderr
                .take()
                .unwrap_or_else(|| Box::new(std::io::stderr()));
            let stdio_client = GRPCStdioClient::new(stdout, stderr);
            tokio::spawn(async move {
                stdio_client.run(channel).await;
            });
        }

        Ok(())
    }

    async fn connect_grpc(&mut self, addr: &str) -> Result<(), PluginError> {
        let plugins = std::mem::take(&mut self.config.plugins);
        let client = GRPCClient::connect(addr, plugins).await?;
        self.grpc_client = Some(client);
        Ok(())
    }

    /// Dispense a plugin interface by name.
    pub fn dispense(&self, name: &str) -> Result<Box<dyn Any + Send>, PluginError> {
        self.grpc_client
            .as_ref()
            .ok_or_else(|| PluginError::Other("client not started".into()))?
            .dispense(name)
    }

    /// Ping the plugin to check if it's alive.
    pub async fn ping(&mut self) -> Result<(), PluginError> {
        self.grpc_client
            .as_mut()
            .ok_or_else(|| PluginError::Other("client not started".into()))?
            .ping()
            .await
    }

    /// Kill the plugin process.
    pub async fn kill(&mut self) {
        if let Some(client) = &mut self.grpc_client {
            let _ = client.shutdown().await;
        }

        if let Some(ref mut child) = self.child {
            let timeout = tokio::time::sleep(GRACEFUL_SHUTDOWN_TIMEOUT);
            tokio::pin!(timeout);

            tokio::select! {
                _ = &mut timeout => {
                    log::warn!("Plugin did not exit gracefully, force-killing");
                    let _ = child.kill();
                }
                _ = async {
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => break,
                            _ => tokio::time::sleep(Duration::from_millis(50)).await,
                        }
                    }
                } => {
                    log::debug!("Plugin exited gracefully");
                }
            }
        }

        self.exited.store(true, Ordering::SeqCst);
    }

    pub fn exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    pub fn negotiated_version(&self) -> Option<u32> {
        self.negotiated_version
    }

    pub fn address(&self) -> Option<&PluginAddr> {
        self.address.as_ref()
    }

    pub fn grpc_client(&self) -> Option<&GRPCClient> {
        self.grpc_client.as_ref()
    }

    pub fn reattach_config(&self) -> Option<ReattachConfig> {
        let addr = self.address.as_ref()?;
        let version = self.negotiated_version?;
        let pid = self.child.as_ref().map(|c| c.id()).unwrap_or(0);
        Some(ReattachConfig {
            protocol: Protocol::Grpc,
            protocol_version: version,
            addr: addr.clone(),
            pid,
        })
    }

    fn supported_versions(&self) -> Vec<u32> {
        if self.config.versioned_plugins.is_empty() {
            vec![self.config.handshake.protocol_version]
        } else {
            let mut versions: Vec<u32> = self.config.versioned_plugins.keys().copied().collect();
            versions.sort_unstable();
            versions
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
        }
    }
}

/// Read plugin stderr lines and forward to the host logger.
fn log_stderr(
    stderr: std::process::ChildStderr,
    mut writer: Option<Box<dyn std::io::Write + Send>>,
    _exited: Arc<AtomicBool>,
) {
    let reader = BufReader::new(stderr);
    for line in reader.lines() {
        match line {
            Ok(line) => {
                if let Some(ref mut w) = writer {
                    let _ = writeln!(w, "{line}");
                }

                if let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) {
                    let level = entry
                        .get("@level")
                        .and_then(|v| v.as_str())
                        .unwrap_or("info");
                    let message = entry
                        .get("@message")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&line);

                    match level {
                        "trace" => log::trace!("[plugin] {message}"),
                        "debug" => log::debug!("[plugin] {message}"),
                        "info" => log::info!("[plugin] {message}"),
                        "warn" => log::warn!("[plugin] {message}"),
                        "error" => log::error!("[plugin] {message}"),
                        _ => log::info!("[plugin] {message}"),
                    }
                } else if line.starts_with("panic:") || line.starts_with("fatal error:") {
                    log::error!("[plugin] {line}");
                } else {
                    log::debug!("[plugin] {line}");
                }
            }
            Err(_) => break,
        }
    }
}

use std::io::Write;

/// Global list of managed clients for cleanup.
static MANAGED_CLIENTS: std::sync::Mutex<Vec<Arc<std::sync::Mutex<Client>>>> =
    std::sync::Mutex::new(Vec::new());

/// Register a client as managed for automatic cleanup.
pub fn register_managed(client: Arc<std::sync::Mutex<Client>>) {
    MANAGED_CLIENTS.lock().unwrap().push(client);
}

/// Kill all managed clients.
pub fn cleanup_clients() {
    let clients = std::mem::take(&mut *MANAGED_CLIENTS.lock().unwrap());
    for client in clients {
        if let Ok(mut c) = client.lock() {
            if let Some(ref mut child) = c.child {
                let _ = child.kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_config_defaults() {
        let config = ClientConfig::default();
        assert!(config.auto_mtls);
        assert_eq!(config.allowed_protocols, vec![Protocol::Grpc]);
        assert_eq!(config.start_timeout, DEFAULT_START_TIMEOUT);
    }

    #[test]
    fn client_not_started_dispense_fails() {
        let client = Client::new(ClientConfig::default());
        assert!(client.dispense("test").is_err());
    }

    #[test]
    fn client_not_started_returns_none_reattach() {
        let client = Client::new(ClientConfig::default());
        assert!(client.reattach_config().is_none());
    }
}
