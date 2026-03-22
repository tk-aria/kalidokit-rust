//! Plugin server implementation.
//!
//! The plugin process calls `serve()` to start listening for host connections.
//! Mirrors Go's `server.go`.

use crate::error::PluginError;
use crate::mtls;
use crate::plugin::PluginSet;
use crate::protocol::{
    self, HandshakeConfig, Protocol, ServerNegotiationLine, CORE_PROTOCOL_VERSION,
    ENV_PLUGIN_CLIENT_CERT, ENV_PLUGIN_MAX_PORT, ENV_PLUGIN_MIN_PORT,
    ENV_PLUGIN_PROTOCOL_VERSIONS, ENV_PLUGIN_UNIX_SOCKET_DIR,
};
use crate::runner::PluginAddr;
use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;

/// Configuration for the plugin server.
pub struct ServeConfig {
    pub handshake: HandshakeConfig,
    pub plugins: PluginSet,
    pub versioned_plugins: HashMap<u32, PluginSet>,
    pub grpc_server_options: Vec<GrpcServerOption>,
    pub test: Option<ServeTestConfig>,
}

/// Options for customizing the gRPC server.
pub enum GrpcServerOption {
    MaxRecvMessageSize(usize),
    MaxSendMessageSize(usize),
}

/// Configuration for running a plugin server in test mode.
pub struct ServeTestConfig {
    pub reattach_config_tx: tokio::sync::mpsc::Sender<crate::runner::ReattachConfig>,
    pub close_rx: tokio::sync::watch::Receiver<bool>,
}

/// Start the plugin server.
pub async fn serve(config: ServeConfig) -> Result<(), PluginError> {
    let is_test = config.test.is_some();

    if !is_test {
        config.handshake.validate_magic_cookie()?;
    }

    // Ignore SIGINT in plugin processes (matches Go behavior)
    #[cfg(unix)]
    {
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_IGN);
        }
    }

    let (app_version, plugins) = negotiate_server_version(&config)?;
    let (listener, addr) = create_listener().await?;

    let server_cert = if std::env::var(ENV_PLUGIN_CLIENT_CERT).is_ok() {
        Some(mtls::generate_server_cert()?)
    } else {
        None
    };

    let neg_line = ServerNegotiationLine {
        core_protocol_version: CORE_PROTOCOL_VERSION,
        app_protocol_version: app_version,
        network: addr.network().to_string(),
        address: addr.address(),
        protocol: Protocol::Grpc,
        server_cert: server_cert.as_ref().map(|c| c.cert_base64()),
        mux_supported: false,
    };

    if let Some(test_config) = &config.test {
        let reattach = crate::runner::ReattachConfig {
            protocol: Protocol::Grpc,
            protocol_version: app_version,
            addr,
            pid: std::process::id(),
        };
        test_config
            .reattach_config_tx
            .send(reattach)
            .await
            .map_err(|e| PluginError::Other(format!("failed to send reattach config: {e}")))?;
    } else {
        let line = neg_line.encode();
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{line}").map_err(|e| {
            PluginError::Other(format!("failed to write negotiation line: {e}"))
        })?;
        stdout.flush().map_err(|e| {
            PluginError::Other(format!("failed to flush stdout: {e}"))
        })?;
    }

    log::info!(
        "Plugin server starting on {} (version={}, protocol=grpc)",
        neg_line.address,
        app_version
    );

    crate::grpc_server::serve_grpc(listener, plugins, server_cert).await
}

fn negotiate_server_version(
    config: &ServeConfig,
) -> Result<(u32, PluginSet), PluginError> {
    if config.versioned_plugins.is_empty() {
        return Ok((config.handshake.protocol_version, config.plugins.clone()));
    }

    let host_versions = match std::env::var(ENV_PLUGIN_PROTOCOL_VERSIONS) {
        Ok(s) => protocol::parse_version_list(&s),
        Err(_) => {
            let min_version = config
                .versioned_plugins
                .keys()
                .copied()
                .min()
                .unwrap_or(config.handshake.protocol_version);
            return Ok((
                min_version,
                config
                    .versioned_plugins
                    .get(&min_version)
                    .cloned()
                    .unwrap_or_default(),
            ));
        }
    };

    let mut server_versions: Vec<u32> = config.versioned_plugins.keys().copied().collect();
    server_versions.sort_unstable();

    for &ver in server_versions.iter().rev() {
        if host_versions.contains(&ver) {
            return Ok((ver, config.versioned_plugins[&ver].clone()));
        }
    }

    Ok((
        config.handshake.protocol_version,
        config.plugins.clone(),
    ))
}

/// Create a listener for plugin connections.
///
/// On Unix, prefers Unix domain sockets (matching Go behavior).
/// Falls back to TCP on Windows or when Unix socket creation fails.
async fn create_listener() -> Result<(TcpListener, PluginAddr), PluginError> {
    // Try Unix socket first on non-Windows
    #[cfg(unix)]
    {
        match create_unix_listener().await {
            Ok((listener, addr)) => return Ok((listener, addr)),
            Err(e) => {
                log::debug!("Unix socket listener failed, falling back to TCP: {e}");
            }
        }
    }

    create_tcp_listener().await
}

/// Create a Unix domain socket listener.
#[cfg(unix)]
async fn create_unix_listener() -> Result<(TcpListener, PluginAddr), PluginError> {
    let socket_dir = std::env::var(ENV_PLUGIN_UNIX_SOCKET_DIR)
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());

    // Create a temporary file path for the socket
    let socket_path = socket_dir.join(format!("plugin-{}.sock", std::process::id()));

    // Remove any existing socket file
    let _ = std::fs::remove_file(&socket_path);

    // Bind Unix listener
    let _unix_listener = tokio::net::UnixListener::bind(&socket_path)
        .map_err(|e| PluginError::Transport(format!("Unix socket bind failed: {e}")))?;

    log::info!("Plugin listening on Unix socket: {:?}", socket_path);

    // We still need a TcpListener for the tonic API.
    // Since tonic supports Unix sockets natively via UDS, we need to
    // bridge this. For now, fall back to TCP and return the Unix addr.
    // The proper solution requires using tonic's UDS support directly.
    Err(PluginError::Transport(
        "Unix socket requires tonic UDS support (falling back to TCP)".into(),
    ))
}

/// Create a TCP listener on localhost.
async fn create_tcp_listener() -> Result<(TcpListener, PluginAddr), PluginError> {
    let min_port: u16 = std::env::var(ENV_PLUGIN_MIN_PORT)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10000);
    let max_port: u16 = std::env::var(ENV_PLUGIN_MAX_PORT)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(25000);

    for port in min_port..=max_port {
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                let local_addr = listener.local_addr()?;
                return Ok((listener, PluginAddr::Tcp(local_addr)));
            }
            Err(_) => continue,
        }
    }

    Err(PluginError::Transport(format!(
        "no available port in range {min_port}..{max_port}"
    )))
}

/// ServeMux map: plugin name -> ServeConfig.
pub type ServeMuxMap = HashMap<String, ServeConfig>;

/// Serve one of multiple plugin configurations based on command-line args.
pub async fn serve_mux(configs: ServeMuxMap) -> Result<(), PluginError> {
    let args: Vec<String> = std::env::args().collect();
    let plugin_name = args.get(1).ok_or_else(|| {
        PluginError::Other(format!(
            "serve_mux requires a plugin name argument; available: {:?}",
            configs.keys().collect::<Vec<_>>()
        ))
    })?;

    let config = configs.into_iter().find(|(k, _)| k == plugin_name);
    match config {
        Some((_, c)) => serve(c).await,
        None => Err(PluginError::PluginNotFound(plugin_name.clone())),
    }
}
