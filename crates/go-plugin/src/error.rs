use thiserror::Error;

/// Errors that can occur in the go-plugin system.
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("protocol negotiation failed: {0}")]
    Negotiation(String),

    #[error("magic cookie mismatch: expected key={key}")]
    MagicCookieMismatch { key: String },

    #[error("core protocol version mismatch: host={host}, plugin={plugin}")]
    CoreProtocolVersionMismatch { host: u32, plugin: u32 },

    #[error("no compatible protocol version: host supports {host:?}, plugin offers {plugin}")]
    NoCompatibleVersion { host: Vec<u32>, plugin: u32 },

    #[error("plugin process exited unexpectedly: {0}")]
    PluginExited(String),

    #[error("plugin start timeout after {0:?}")]
    StartTimeout(std::time::Duration),

    #[error("plugin not found: {0}")]
    PluginNotFound(String),

    #[error("subprocess error: {0}")]
    Subprocess(String),

    #[error("transport error: {0}")]
    Transport(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("checksum verification failed")]
    ChecksumMismatch,

    #[error("broker error: {0}")]
    Broker(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("grpc error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("grpc transport error: {0}")]
    GrpcTransport(#[from] tonic::transport::Error),

    #[error("{0}")]
    Other(String),
}

/// A serializable error type for passing errors across RPC boundaries.
/// Mirrors Go's `plugin.BasicError`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BasicError {
    pub message: String,
}

impl std::fmt::Display for BasicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BasicError {}

impl From<String> for BasicError {
    fn from(message: String) -> Self {
        Self { message }
    }
}
