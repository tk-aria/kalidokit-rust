//! Plugin protocol definitions.
//!
//! Implements the wire protocol for communication between host and plugin
//! processes. The protocol negotiation happens over stdout: the plugin writes
//! a single pipe-delimited line containing connection information.
//!
//! Format: `{CoreProtocolVersion}|{AppProtocolVersion}|{Network}|{Address}|{Protocol}|{ServerCert}|{MuxSupported}`

use crate::error::PluginError;
use std::fmt;
use std::str::FromStr;

/// The core protocol version. Must match between host and plugin.
/// This version is independent of the application-level protocol version.
pub const CORE_PROTOCOL_VERSION: u32 = 1;

/// Environment variable name for the minimum port in the plugin's listener range.
pub const ENV_PLUGIN_MIN_PORT: &str = "PLUGIN_MIN_PORT";

/// Environment variable name for the maximum port in the plugin's listener range.
pub const ENV_PLUGIN_MAX_PORT: &str = "PLUGIN_MAX_PORT";

/// Environment variable name for the comma-separated list of protocol versions.
pub const ENV_PLUGIN_PROTOCOL_VERSIONS: &str = "PLUGIN_PROTOCOL_VERSIONS";

/// Environment variable name for the client certificate (PEM, base64).
pub const ENV_PLUGIN_CLIENT_CERT: &str = "PLUGIN_CLIENT_CERT";

/// Environment variable name to enable gRPC broker multiplexing.
pub const ENV_PLUGIN_MULTIPLEX_GRPC: &str = "PLUGIN_MULTIPLEX_GRPC";

/// Environment variable name for the Unix socket directory.
pub const ENV_PLUGIN_UNIX_SOCKET_DIR: &str = "PLUGIN_UNIX_SOCKET_DIR";

/// Environment variable name for the Unix socket group ownership.
pub const ENV_PLUGIN_UNIX_SOCKET_GROUP: &str = "PLUGIN_UNIX_SOCKET_GROUP";

/// The transport protocol used between host and plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// gRPC transport (recommended, cross-language).
    Grpc,
    /// net/rpc transport (Go-compatible, uses yamux + msgpack).
    NetRpc,
}

impl Protocol {
    /// Wire format string for the negotiation line.
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Grpc => "grpc",
            Protocol::NetRpc => "netrpc",
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Protocol {
    type Err = PluginError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "grpc" => Ok(Protocol::Grpc),
            "netrpc" => Ok(Protocol::NetRpc),
            _ => Err(PluginError::Negotiation(format!(
                "unknown protocol: {s:?}"
            ))),
        }
    }
}

/// Configuration for the handshake between host and plugin.
///
/// The magic cookie is NOT a security mechanism -- it prevents users from
/// accidentally executing plugin binaries directly. The host sets the cookie
/// as an env var; the plugin checks it at startup.
#[derive(Debug, Clone)]
pub struct HandshakeConfig {
    /// Application-level protocol version.
    pub protocol_version: u32,
    /// Environment variable name for the magic cookie.
    pub magic_cookie_key: String,
    /// Expected value of the magic cookie.
    pub magic_cookie_value: String,
}

impl HandshakeConfig {
    /// Validate that the magic cookie environment variable is set correctly.
    /// Called by the plugin process on startup.
    pub fn validate_magic_cookie(&self) -> Result<(), PluginError> {
        match std::env::var(&self.magic_cookie_key) {
            Ok(val) if val == self.magic_cookie_value => Ok(()),
            _ => Err(PluginError::MagicCookieMismatch {
                key: self.magic_cookie_key.clone(),
            }),
        }
    }
}

/// The negotiation line written by the plugin to stdout.
///
/// Contains all information the host needs to connect to the plugin.
#[derive(Debug, Clone)]
pub struct ServerNegotiationLine {
    /// Core protocol version (must be `CORE_PROTOCOL_VERSION`).
    pub core_protocol_version: u32,
    /// Application-level protocol version negotiated with the host.
    pub app_protocol_version: u32,
    /// Network type: `"unix"` or `"tcp"`.
    pub network: String,
    /// Listener address (Unix socket path or `host:port`).
    pub address: String,
    /// Transport protocol.
    pub protocol: Protocol,
    /// Base64-encoded DER server certificate (for AutoMTLS).
    pub server_cert: Option<String>,
    /// Whether gRPC broker multiplexing is supported.
    pub mux_supported: bool,
}

impl ServerNegotiationLine {
    /// Parse a negotiation line from the plugin's stdout.
    ///
    /// The format is pipe-delimited:
    /// `{core_ver}|{app_ver}|{network}|{address}|{protocol}|{server_cert}|{mux_supported}`
    ///
    /// Fields 5 (server_cert) and 6 (mux_supported) are optional.
    pub fn parse(line: &str) -> Result<Self, PluginError> {
        let line = line.trim();
        let parts: Vec<&str> = line.split('|').collect();

        if parts.len() < 5 {
            return Err(PluginError::Negotiation(format!(
                "expected at least 5 pipe-delimited fields, got {}: {line:?}",
                parts.len()
            )));
        }

        let core_protocol_version = parts[0].parse::<u32>().map_err(|e| {
            PluginError::Negotiation(format!("invalid core protocol version: {e}"))
        })?;

        let app_protocol_version = parts[1].parse::<u32>().map_err(|e| {
            PluginError::Negotiation(format!("invalid app protocol version: {e}"))
        })?;

        let network = parts[2].to_string();
        let address = parts[3].to_string();
        let protocol = parts[4].parse::<Protocol>()?;

        let server_cert = parts.get(5).and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        });

        let mux_supported = parts
            .get(6)
            .map(|s| *s == "true")
            .unwrap_or(false);

        Ok(Self {
            core_protocol_version,
            app_protocol_version,
            network,
            address,
            protocol,
            server_cert,
            mux_supported,
        })
    }

    /// Encode this negotiation line to the wire format.
    pub fn encode(&self) -> String {
        let cert = self.server_cert.as_deref().unwrap_or("");
        if self.mux_supported {
            format!(
                "{}|{}|{}|{}|{}|{}|true",
                self.core_protocol_version,
                self.app_protocol_version,
                self.network,
                self.address,
                self.protocol.as_str(),
                cert,
            )
        } else if !cert.is_empty() {
            format!(
                "{}|{}|{}|{}|{}|{}",
                self.core_protocol_version,
                self.app_protocol_version,
                self.network,
                self.address,
                self.protocol.as_str(),
                cert,
            )
        } else {
            format!(
                "{}|{}|{}|{}|{}",
                self.core_protocol_version,
                self.app_protocol_version,
                self.network,
                self.address,
                self.protocol.as_str(),
            )
        }
    }

    /// Validate the core protocol version.
    pub fn validate_core_version(&self) -> Result<(), PluginError> {
        if self.core_protocol_version != CORE_PROTOCOL_VERSION {
            return Err(PluginError::CoreProtocolVersionMismatch {
                host: CORE_PROTOCOL_VERSION,
                plugin: self.core_protocol_version,
            });
        }
        Ok(())
    }
}

/// Negotiate the application-level protocol version.
///
/// Both host and plugin provide lists of supported versions. The highest
/// mutually supported version is selected (matching Go's behavior of
/// iterating in reverse).
pub fn negotiate_version(
    host_versions: &[u32],
    plugin_version: u32,
) -> Result<u32, PluginError> {
    // The Go implementation sends all supported versions as
    // PLUGIN_PROTOCOL_VERSIONS env var. The plugin picks the highest match.
    // On the host side, we check if the plugin's chosen version is acceptable.
    if host_versions.contains(&plugin_version) {
        Ok(plugin_version)
    } else {
        Err(PluginError::NoCompatibleVersion {
            host: host_versions.to_vec(),
            plugin: plugin_version,
        })
    }
}

/// Format protocol versions as a comma-separated string for the
/// `PLUGIN_PROTOCOL_VERSIONS` environment variable.
pub fn encode_version_list(versions: &[u32]) -> String {
    versions
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Parse the `PLUGIN_PROTOCOL_VERSIONS` environment variable.
pub fn parse_version_list(s: &str) -> Vec<u32> {
    s.split(',')
        .filter_map(|v| v.trim().parse::<u32>().ok())
        .collect()
}

/// Server-side protocol abstraction.
///
/// Implemented by `GRPCServer` and `RPCServer` to provide a common
/// interface for different transport protocols.
///
/// Mirrors Go's `ServerProtocol` interface.
#[tonic::async_trait]
pub trait ServerProtocol: Send + Sync {
    /// Initialize the server (called before serving).
    async fn init(&self) -> Result<(), crate::error::PluginError>;

    /// Return the configuration string (for the negotiation line).
    fn config(&self) -> String;

    /// Start serving on the given listener.
    async fn serve(&self, listener: tokio::net::TcpListener) -> Result<(), crate::error::PluginError>;
}

/// Client-side protocol abstraction.
///
/// Implemented by `GRPCClient` and `RPCClient` to provide a common
/// interface for different transport protocols.
///
/// Mirrors Go's `ClientProtocol` interface.
#[tonic::async_trait]
pub trait ClientProtocol: Send + Sync {
    /// Dispense a plugin interface by name.
    fn dispense(&self, name: &str) -> Result<Box<dyn std::any::Any + Send>, crate::error::PluginError>;

    /// Ping the plugin to check if it's alive.
    async fn ping(&mut self) -> Result<(), crate::error::PluginError>;

    /// Close the connection to the plugin.
    async fn close(&mut self) -> Result<(), crate::error::PluginError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_negotiation_line_grpc() {
        let line = "1|5|unix|/tmp/plugin.sock|grpc|dGVzdGNlcnQ=|true";
        let neg = ServerNegotiationLine::parse(line).unwrap();
        assert_eq!(neg.core_protocol_version, 1);
        assert_eq!(neg.app_protocol_version, 5);
        assert_eq!(neg.network, "unix");
        assert_eq!(neg.address, "/tmp/plugin.sock");
        assert_eq!(neg.protocol, Protocol::Grpc);
        assert_eq!(neg.server_cert, Some("dGVzdGNlcnQ=".to_string()));
        assert!(neg.mux_supported);
    }

    #[test]
    fn parse_negotiation_line_netrpc() {
        let line = "1|1|tcp|127.0.0.1:12345|netrpc";
        let neg = ServerNegotiationLine::parse(line).unwrap();
        assert_eq!(neg.core_protocol_version, 1);
        assert_eq!(neg.app_protocol_version, 1);
        assert_eq!(neg.network, "tcp");
        assert_eq!(neg.address, "127.0.0.1:12345");
        assert_eq!(neg.protocol, Protocol::NetRpc);
        assert_eq!(neg.server_cert, None);
        assert!(!neg.mux_supported);
    }

    #[test]
    fn roundtrip_negotiation_line() {
        let original = ServerNegotiationLine {
            core_protocol_version: 1,
            app_protocol_version: 3,
            network: "unix".to_string(),
            address: "/tmp/test.sock".to_string(),
            protocol: Protocol::Grpc,
            server_cert: Some("Y2VydA==".to_string()),
            mux_supported: true,
        };
        let encoded = original.encode();
        let parsed = ServerNegotiationLine::parse(&encoded).unwrap();
        assert_eq!(parsed.core_protocol_version, original.core_protocol_version);
        assert_eq!(parsed.app_protocol_version, original.app_protocol_version);
        assert_eq!(parsed.network, original.network);
        assert_eq!(parsed.address, original.address);
        assert_eq!(parsed.protocol, original.protocol);
        assert_eq!(parsed.server_cert, original.server_cert);
        assert_eq!(parsed.mux_supported, original.mux_supported);
    }

    #[test]
    fn roundtrip_minimal() {
        let original = ServerNegotiationLine {
            core_protocol_version: 1,
            app_protocol_version: 1,
            network: "tcp".to_string(),
            address: "127.0.0.1:5000".to_string(),
            protocol: Protocol::NetRpc,
            server_cert: None,
            mux_supported: false,
        };
        let encoded = original.encode();
        assert_eq!(encoded, "1|1|tcp|127.0.0.1:5000|netrpc");
        let parsed = ServerNegotiationLine::parse(&encoded).unwrap();
        assert_eq!(parsed.protocol, Protocol::NetRpc);
        assert_eq!(parsed.server_cert, None);
        assert!(!parsed.mux_supported);
    }

    #[test]
    fn parse_too_few_fields() {
        let line = "1|5|unix|/tmp/sock";
        assert!(ServerNegotiationLine::parse(line).is_err());
    }

    #[test]
    fn parse_bad_protocol() {
        let line = "1|5|unix|/tmp/sock|http";
        assert!(ServerNegotiationLine::parse(line).is_err());
    }

    #[test]
    fn validate_core_version_ok() {
        let neg = ServerNegotiationLine {
            core_protocol_version: CORE_PROTOCOL_VERSION,
            app_protocol_version: 1,
            network: "tcp".into(),
            address: "127.0.0.1:1234".into(),
            protocol: Protocol::Grpc,
            server_cert: None,
            mux_supported: false,
        };
        assert!(neg.validate_core_version().is_ok());
    }

    #[test]
    fn validate_core_version_mismatch() {
        let neg = ServerNegotiationLine {
            core_protocol_version: 99,
            app_protocol_version: 1,
            network: "tcp".into(),
            address: "127.0.0.1:1234".into(),
            protocol: Protocol::Grpc,
            server_cert: None,
            mux_supported: false,
        };
        assert!(neg.validate_core_version().is_err());
    }

    #[test]
    fn negotiate_version_success() {
        assert_eq!(negotiate_version(&[1, 2, 3], 2).unwrap(), 2);
    }

    #[test]
    fn negotiate_version_failure() {
        assert!(negotiate_version(&[1, 2], 5).is_err());
    }

    #[test]
    fn encode_parse_version_list() {
        let versions = vec![1, 2, 5];
        let encoded = encode_version_list(&versions);
        assert_eq!(encoded, "1,2,5");
        let parsed = parse_version_list(&encoded);
        assert_eq!(parsed, versions);
    }
}
