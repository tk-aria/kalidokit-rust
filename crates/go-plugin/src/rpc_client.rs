//! net/rpc client implementation over yamux.
//!
//! Provides a client that connects to a net/rpc server over a yamux-multiplexed
//! connection. Implements control (ping/quit) and dispense RPCs using msgpack
//! serialization.
//!
//! Mirrors Go's `rpc_client.go`.

use crate::error::PluginError;
use crate::mux_broker::MuxBroker;
use crate::rpc_codec::{self, methods, RpcRequest, RpcResponse};
use futures::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// net/rpc client that communicates with a plugin over yamux.
///
/// Sends msgpack-encoded RPC requests and receives responses over
/// the control stream. Additional streams are opened via the MuxBroker.
pub struct RPCClient {
    /// The MuxBroker for multiplexed streams.
    broker: Arc<MuxBroker>,
    /// The control stream (stream ID 0) for RPC calls.
    control_stream: tokio::sync::Mutex<yamux::Stream>,
    /// Sequence counter for RPC requests.
    seq: AtomicU64,
}

impl RPCClient {
    /// Create a new RPCClient with the given broker and control stream.
    pub fn new(broker: Arc<MuxBroker>, control_stream: yamux::Stream) -> Self {
        Self {
            broker,
            control_stream: tokio::sync::Mutex::new(control_stream),
            seq: AtomicU64::new(1),
        }
    }

    /// Get the next sequence number.
    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    /// Send an RPC request and receive the response.
    async fn call<T: serde::Serialize, R: for<'de> serde::Deserialize<'de>>(
        &self,
        method: &str,
        body: &T,
    ) -> Result<R, PluginError> {
        let seq = self.next_seq();
        let req = RpcRequest {
            service_method: method.to_string(),
            seq,
        };

        let encoded = rpc_codec::encode_request(&req, body)?;
        let mut stream = self.control_stream.lock().await;

        // Write length-prefixed message
        let len_bytes = (encoded.len() as u32).to_be_bytes();
        stream
            .write_all(&len_bytes)
            .await
            .map_err(|e| PluginError::Transport(format!("rpc write request: {e}")))?;
        stream
            .write_all(&encoded)
            .await
            .map_err(|e| PluginError::Transport(format!("rpc write request body: {e}")))?;
        stream
            .flush()
            .await
            .map_err(|e| PluginError::Transport(format!("rpc flush: {e}")))?;

        // Read response length
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| PluginError::Transport(format!("rpc read response length: {e}")))?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        if resp_len > 10 * 1024 * 1024 {
            return Err(PluginError::Transport(format!(
                "rpc response too large: {resp_len} bytes"
            )));
        }

        // Read response body
        let mut resp_buf = vec![0u8; resp_len];
        stream
            .read_exact(&mut resp_buf)
            .await
            .map_err(|e| PluginError::Transport(format!("rpc read response body: {e}")))?;

        drop(stream);

        let mut cursor = std::io::Cursor::new(&resp_buf);
        let resp: RpcResponse = rpc_codec::decode_response_header(&mut cursor)?;

        if resp.seq != seq {
            return Err(PluginError::Transport(format!(
                "rpc sequence mismatch: expected {seq}, got {}",
                resp.seq
            )));
        }

        if !resp.error.is_empty() {
            return Err(PluginError::Transport(format!(
                "rpc error from {}: {}",
                method, resp.error
            )));
        }

        let result: R = rpc_codec::decode_body(&mut cursor)?;
        Ok(result)
    }

    /// Ping the plugin to check if it's alive.
    pub async fn ping(&self) -> Result<(), PluginError> {
        let _: () = self.call(methods::CONTROL_PING, &()).await?;
        Ok(())
    }

    /// Request graceful shutdown.
    pub async fn quit(&self) -> Result<(), PluginError> {
        let _: () = self.call(methods::CONTROL_QUIT, &()).await?;
        Ok(())
    }

    /// Dispense a plugin by name, returning the broker stream ID.
    pub async fn dispense(&self, name: &str) -> Result<u32, PluginError> {
        let stream_id: u32 = self.call(methods::DISPENSER_DISPENSE, &name).await?;
        Ok(stream_id)
    }

    /// Get a reference to the broker.
    pub fn broker(&self) -> &Arc<MuxBroker> {
        &self.broker
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_client_seq_increments() {
        let seq = AtomicU64::new(1);
        assert_eq!(seq.fetch_add(1, Ordering::SeqCst), 1);
        assert_eq!(seq.fetch_add(1, Ordering::SeqCst), 2);
        assert_eq!(seq.fetch_add(1, Ordering::SeqCst), 3);
    }
}
