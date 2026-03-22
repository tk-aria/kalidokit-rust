//! msgpack-based RPC codec for net/rpc transport.
//!
//! Implements the wire format compatible with Go's `net/rpc` using
//! msgpack serialization. Each RPC message has a header followed by
//! a msgpack-encoded body.
//!
//! Mirrors Go's `rpc_codec.go` (MuxBrokerRWC + msgpackCodec).

use crate::error::PluginError;
use serde::{Deserialize, Serialize};

/// RPC request header, compatible with Go's `rpc.Request`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// Service.Method name (e.g., "Plugin.Dispense").
    #[serde(rename = "ServiceMethod")]
    pub service_method: String,
    /// Sequence number.
    #[serde(rename = "Seq")]
    pub seq: u64,
}

/// RPC response header, compatible with Go's `rpc.Response`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    /// Service.Method name.
    #[serde(rename = "ServiceMethod")]
    pub service_method: String,
    /// Sequence number matching the request.
    #[serde(rename = "Seq")]
    pub seq: u64,
    /// Error string (empty means success).
    #[serde(rename = "Error")]
    pub error: String,
}

/// Encode an RPC request header + body to msgpack bytes.
pub fn encode_request<T: Serialize>(
    req: &RpcRequest,
    body: &T,
) -> Result<Vec<u8>, PluginError> {
    let mut buf = Vec::new();
    // Header
    rmp_serde::encode::write(&mut buf, req)
        .map_err(|e| PluginError::Transport(format!("encode rpc request header: {e}")))?;
    // Body
    rmp_serde::encode::write(&mut buf, body)
        .map_err(|e| PluginError::Transport(format!("encode rpc request body: {e}")))?;
    Ok(buf)
}

/// Encode an RPC response header + body to msgpack bytes.
pub fn encode_response<T: Serialize>(
    resp: &RpcResponse,
    body: &T,
) -> Result<Vec<u8>, PluginError> {
    let mut buf = Vec::new();
    // Header
    rmp_serde::encode::write(&mut buf, resp)
        .map_err(|e| PluginError::Transport(format!("encode rpc response header: {e}")))?;
    // Body
    rmp_serde::encode::write(&mut buf, body)
        .map_err(|e| PluginError::Transport(format!("encode rpc response body: {e}")))?;
    Ok(buf)
}

/// Decode an RPC request header from a msgpack reader.
pub fn decode_request_header<R: std::io::Read>(
    reader: &mut R,
) -> Result<RpcRequest, PluginError> {
    rmp_serde::from_read(reader)
        .map_err(|e| PluginError::Transport(format!("decode rpc request header: {e}")))
}

/// Decode an RPC response header from a msgpack reader.
pub fn decode_response_header<R: std::io::Read>(
    reader: &mut R,
) -> Result<RpcResponse, PluginError> {
    rmp_serde::from_read(reader)
        .map_err(|e| PluginError::Transport(format!("decode rpc response header: {e}")))
}

/// Decode a msgpack body value from a reader.
pub fn decode_body<R: std::io::Read, T: for<'de> Deserialize<'de>>(
    reader: &mut R,
) -> Result<T, PluginError> {
    rmp_serde::from_read(reader)
        .map_err(|e| PluginError::Transport(format!("decode rpc body: {e}")))
}

/// Well-known net/rpc service method names used by go-plugin.
pub mod methods {
    /// Control.Ping — health check.
    pub const CONTROL_PING: &str = "Control.Ping";
    /// Control.Quit — graceful shutdown.
    pub const CONTROL_QUIT: &str = "Control.Quit";
    /// Dispenser.Dispense — get a plugin interface by name.
    pub const DISPENSER_DISPENSE: &str = "Dispenser.Dispense";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_request() {
        let req = RpcRequest {
            service_method: "Plugin.Dispense".into(),
            seq: 1,
        };
        let body = "greeter";
        let encoded = encode_request(&req, &body).unwrap();

        let mut reader = &encoded[..];
        let decoded_req: RpcRequest = decode_request_header(&mut reader).unwrap();
        let decoded_body: String = decode_body(&mut reader).unwrap();

        assert_eq!(decoded_req.service_method, "Plugin.Dispense");
        assert_eq!(decoded_req.seq, 1);
        assert_eq!(decoded_body, "greeter");
    }

    #[test]
    fn roundtrip_response() {
        let resp = RpcResponse {
            service_method: "Plugin.Dispense".into(),
            seq: 1,
            error: String::new(),
        };
        let body = 42u32;
        let encoded = encode_response(&resp, &body).unwrap();

        let mut reader = &encoded[..];
        let decoded_resp: RpcResponse = decode_response_header(&mut reader).unwrap();
        let decoded_body: u32 = decode_body(&mut reader).unwrap();

        assert_eq!(decoded_resp.service_method, "Plugin.Dispense");
        assert_eq!(decoded_resp.seq, 1);
        assert!(decoded_resp.error.is_empty());
        assert_eq!(decoded_body, 42);
    }

    #[test]
    fn response_with_error() {
        let resp = RpcResponse {
            service_method: "Control.Ping".into(),
            seq: 2,
            error: "plugin not found".into(),
        };
        let body = ();
        let encoded = encode_response(&resp, &body).unwrap();

        let mut reader = &encoded[..];
        let decoded: RpcResponse = decode_response_header(&mut reader).unwrap();
        assert_eq!(decoded.error, "plugin not found");
    }
}
