//! Plugin subprocess runner.
//!
//! Defines the `Runner` trait for managing plugin process lifecycle and
//! provides `CmdRunner`, the default implementation that wraps
//! `std::process::Command`.

use crate::error::PluginError;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Trait for managing the lifecycle of a plugin subprocess.
///
/// Mirrors Go's `runner.Runner` interface.
pub trait Runner: Send + Sync {
    /// Start the plugin process.
    /// Returns the running child process handle.
    fn start(&self) -> Result<RunningPlugin, PluginError>;

    /// Human-readable name of the plugin (typically the binary name).
    fn name(&self) -> &str;

    /// Diagnostic information about the runner configuration.
    fn diagnose(&self) -> String;
}

/// A running plugin process with captured stdio pipes.
pub struct RunningPlugin {
    pub child: Child,
    pub stdout: BufReader<std::process::ChildStdout>,
    pub stderr: Option<std::process::ChildStderr>,
}

impl RunningPlugin {
    /// Read the first line from stdout (the protocol negotiation line).
    ///
    /// This blocks until a line is available or EOF.
    pub fn read_negotiation_line(&mut self) -> Result<String, PluginError> {
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line).map_err(|e| {
            PluginError::Negotiation(format!("failed to read negotiation line: {e}"))
        })?;
        if n == 0 {
            return Err(PluginError::Negotiation(
                "plugin closed stdout before writing negotiation line".into(),
            ));
        }
        Ok(line)
    }

    /// Wait for the plugin process to exit.
    pub fn wait(&mut self) -> Result<std::process::ExitStatus, PluginError> {
        self.child.wait().map_err(PluginError::Io)
    }

    /// Kill the plugin process.
    pub fn kill(&mut self) -> Result<(), PluginError> {
        match self.child.kill() {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {
                // Process already exited -- not an error
                Ok(())
            }
            Err(e) => Err(PluginError::Io(e)),
        }
    }

    /// Check if the process has exited without blocking.
    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, PluginError> {
        self.child.try_wait().map_err(PluginError::Io)
    }

    /// Get the process ID.
    pub fn id(&self) -> u32 {
        self.child.id()
    }
}

/// Default plugin subprocess runner using `std::process::Command`.
///
/// Mirrors Go's `CmdRunner` in `internal/cmdrunner/`.
#[derive(Debug, Clone)]
pub struct CmdRunner {
    /// Path to the plugin binary.
    pub cmd: PathBuf,
    /// Arguments to pass to the plugin binary.
    pub args: Vec<String>,
    /// Additional environment variables to set.
    pub env: HashMap<String, String>,
    /// Working directory for the subprocess (None = inherit).
    pub dir: Option<PathBuf>,
}

impl CmdRunner {
    pub fn new(cmd: impl Into<PathBuf>) -> Self {
        Self {
            cmd: cmd.into(),
            args: Vec::new(),
            dir: None,
            env: HashMap::new(),
        }
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn envs(
        mut self,
        vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (k, v) in vars {
            self.env.insert(k.into(), v.into());
        }
        self
    }

    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dir = Some(dir.into());
        self
    }
}

impl Runner for CmdRunner {
    fn start(&self) -> Result<RunningPlugin, PluginError> {
        let mut cmd = Command::new(&self.cmd);
        cmd.args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (k, v) in &self.env {
            cmd.env(k, v);
        }

        if let Some(dir) = &self.dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().map_err(|e| {
            PluginError::Subprocess(format!(
                "failed to start plugin {:?}: {e}",
                self.cmd
            ))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            PluginError::Subprocess("failed to capture plugin stdout".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            PluginError::Subprocess("failed to capture plugin stderr".into())
        })?;

        log::debug!(
            "Plugin process started: {:?} (pid={})",
            self.cmd,
            child.id()
        );

        Ok(RunningPlugin {
            child,
            stdout: BufReader::new(stdout),
            stderr: Some(stderr),
        })
    }

    fn name(&self) -> &str {
        self.cmd
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }

    fn diagnose(&self) -> String {
        format!(
            "CmdRunner {{ cmd: {:?}, args: {:?}, env_count: {} }}",
            self.cmd,
            self.args,
            self.env.len()
        )
    }
}

/// Configuration for reattaching to an already-running plugin process.
///
/// Mirrors Go's `ReattachConfig`.
#[derive(Debug, Clone)]
pub struct ReattachConfig {
    /// Transport protocol the plugin is using.
    pub protocol: crate::protocol::Protocol,
    /// Application-level protocol version.
    pub protocol_version: u32,
    /// Network address of the plugin's listener.
    pub addr: PluginAddr,
    /// Process ID of the running plugin (for monitoring).
    pub pid: u32,
}

/// Network address of a plugin listener.
#[derive(Debug, Clone)]
pub enum PluginAddr {
    /// Unix domain socket path.
    Unix(PathBuf),
    /// TCP address.
    Tcp(std::net::SocketAddr),
}

impl PluginAddr {
    /// Network type string for the negotiation line.
    pub fn network(&self) -> &str {
        match self {
            PluginAddr::Unix(_) => "unix",
            PluginAddr::Tcp(_) => "tcp",
        }
    }

    /// Address string for the negotiation line.
    pub fn address(&self) -> String {
        match self {
            PluginAddr::Unix(p) => p.to_string_lossy().to_string(),
            PluginAddr::Tcp(a) => a.to_string(),
        }
    }

    /// Parse from network type and address strings.
    pub fn parse(network: &str, address: &str) -> Result<Self, PluginError> {
        match network {
            "unix" => Ok(PluginAddr::Unix(PathBuf::from(address))),
            "tcp" => {
                let addr = address.parse().map_err(|e| {
                    PluginError::Negotiation(format!("invalid TCP address {address:?}: {e}"))
                })?;
                Ok(PluginAddr::Tcp(addr))
            }
            _ => Err(PluginError::Negotiation(format!(
                "unknown network type: {network:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_runner_starts_process() {
        let runner = CmdRunner::new("echo").args(["hello"]);
        let mut plugin = runner.start().unwrap();
        let line = plugin.read_negotiation_line().unwrap();
        assert_eq!(line.trim(), "hello");
        let status = plugin.wait().unwrap();
        assert!(status.success());
    }

    #[test]
    fn cmd_runner_with_env() {
        let runner = CmdRunner::new("sh")
            .args(["-c", "echo $TEST_VAR"])
            .env("TEST_VAR", "plugin_test_value");
        let mut plugin = runner.start().unwrap();
        let line = plugin.read_negotiation_line().unwrap();
        assert_eq!(line.trim(), "plugin_test_value");
    }

    #[test]
    fn cmd_runner_kill() {
        let runner = CmdRunner::new("sleep").args(["60"]);
        let mut plugin = runner.start().unwrap();
        assert!(plugin.try_wait().unwrap().is_none()); // still running
        plugin.kill().unwrap();
        let status = plugin.wait().unwrap();
        assert!(!status.success()); // killed
    }

    #[test]
    fn cmd_runner_nonexistent_binary() {
        let runner = CmdRunner::new("/nonexistent/binary");
        assert!(runner.start().is_err());
    }

    #[test]
    fn cmd_runner_name() {
        let runner = CmdRunner::new("/usr/bin/my-plugin");
        assert_eq!(runner.name(), "my-plugin");
    }

    #[test]
    fn plugin_addr_roundtrip_unix() {
        let addr = PluginAddr::Unix(PathBuf::from("/tmp/test.sock"));
        assert_eq!(addr.network(), "unix");
        assert_eq!(addr.address(), "/tmp/test.sock");
        let parsed = PluginAddr::parse(addr.network(), &addr.address()).unwrap();
        assert!(matches!(parsed, PluginAddr::Unix(p) if p == PathBuf::from("/tmp/test.sock")));
    }

    #[test]
    fn plugin_addr_roundtrip_tcp() {
        let addr = PluginAddr::Tcp("127.0.0.1:5000".parse().unwrap());
        assert_eq!(addr.network(), "tcp");
        assert_eq!(addr.address(), "127.0.0.1:5000");
        let parsed = PluginAddr::parse(addr.network(), &addr.address()).unwrap();
        assert!(matches!(parsed, PluginAddr::Tcp(a) if a.port() == 5000));
    }
}
