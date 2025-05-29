//! net/rpc server implementation over yamux.
//!
//! Provides a server that accepts yamux-multiplexed connections and handles
//! Go-compatible net/rpc calls using msgpack serialization. Implements the
//! control and dispenser protocols that go-plugin expects.
//!
//! Mirrors Go's `rpc_server.go`.

use crate::error::PluginError;
use crate::mux_broker::MuxBroker;
use crate::plugin::PluginSet;
use crate::rpc_codec::{self, methods, RpcRequest, RpcResponse};
use std::sync::Arc;
use tokio::sync::watch;

/// net/rpc server that handles plugin control and dispense calls.
///
/// Works over a yamux-multiplexed connection using msgpack encoding.
/// The main stream (stream 0) handles control RPCs; additional streams
/// are opened via the MuxBroker for plugin-specific communication.
pub struct RPCServer {
    /// The MuxBroker for multiplexed streams.
    broker: Arc<MuxBroker>,
    /// Registered plugins.
    plugins: PluginSet,
    /// Shutdown signal sender.
    shutdown_tx: watch::Sender<bool>,
    /// Shutdown signal receiver.
    shutdown_rx: watch::Receiver<bool>,
}

impl RPCServer {
    /// Create a new RPCServer with the given broker and plugins.
    pub fn new(broker: Arc<MuxBroker>, plugins: PluginSet) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            broker,
            plugins,
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Serve RPC requests on the given yamux stream.
    ///
    /// Reads msgpack-encoded RPC request headers and bodies, dispatches
    /// to the appropriate handler, and writes back responses.
    pub async fn serve_conn(&self, mut stream: yamux::Stream) -> Result<(), PluginError> {
        use futures::{AsyncReadExt, AsyncWriteExt};
        use std::io::Cursor;

        loop {
            // Check shutdown
            if *self.shutdown_rx.borrow() {
                break;
            }

            // Read request data
            // Go's msgpack codec reads header then body from the stream.
            // We read into a buffer first.
            let mut len_buf = [0u8; 4];
            match futures::AsyncReadExt::read(&mut stream, &mut len_buf).await {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(e) => {
                    log::debug!("RPC stream read error: {e}");
                    break;
                }
            }
            let msg_len = u32::from_be_bytes(len_buf) as usize;

            if msg_len > 10 * 1024 * 1024 {
                log::warn!("RPC message too large: {msg_len} bytes");
                break;
            }

            let mut msg_buf = vec![0u8; msg_len];
            if let Err(e) = stream.read_exact(&mut msg_buf).await {
                log::debug!("RPC read message body error: {e}");
                break;
            }

            let mut cursor = Cursor::new(&msg_buf);
            let req = match rpc_codec::decode_request_header(&mut cursor) {
                Ok(r) => r,
                Err(e) => {
                    log::debug!("RPC decode request error: {e}");
                    break;
                }
            };

            let (resp, resp_body) = self.handle_request(&req, &mut cursor).await;

            // Encode and send response
            let encoded = match rpc_codec::encode_response(&resp, &resp_body) {
                Ok(e) => e,
                Err(e) => {
                    log::debug!("RPC encode response error: {e}");
                    break;
                }
            };

            let len_bytes = (encoded.len() as u32).to_be_bytes();
            if let Err(e) = stream.write_all(&len_bytes).await {
                log::debug!("RPC write response length error: {e}");
                break;
            }
            if let Err(e) = stream.write_all(&encoded).await {
                log::debug!("RPC write response error: {e}");
                break;
            }
            let _ = stream.flush().await;

            // If this was a quit command, exit
            if req.service_method == methods::CONTROL_QUIT {
                break;
            }
        }

        Ok(())
    }

    /// Handle a single RPC request, dispatching to the appropriate handler.
    async fn handle_request(
        &self,
        req: &RpcRequest,
        body_reader: &mut std::io::Cursor<&Vec<u8>>,
    ) -> (RpcResponse, Vec<u8>) {
        match req.service_method.as_str() {
            methods::CONTROL_PING => {
                let resp = RpcResponse {
                    service_method: req.service_method.clone(),
                    seq: req.seq,
                    error: String::new(),
                };
                let body = rmp_serde::to_vec(&()).unwrap_or_default();
                (resp, body)
            }
            methods::CONTROL_QUIT => {
                self.shutdown_tx.send(true).ok();
                let resp = RpcResponse {
                    service_method: req.service_method.clone(),
                    seq: req.seq,
                    error: String::new(),
                };
                let body = rmp_serde::to_vec(&()).unwrap_or_default();
                (resp, body)
            }
            methods::DISPENSER_DISPENSE => {
                let name: Result<String, _> = rpc_codec::decode_body(body_reader);
                match name {
                    Ok(name) => {
                        if self.plugins.contains_key(&name) {
                            // Return the broker stream ID for the plugin
                            let stream_id = self.broker.next_id();
                            let resp = RpcResponse {
                                service_method: req.service_method.clone(),
                                seq: req.seq,
                                error: String::new(),
                            };
                            let body = rmp_serde::to_vec(&stream_id).unwrap_or_default();
                            (resp, body)
                        } else {
                            let resp = RpcResponse {
                                service_method: req.service_method.clone(),
                                seq: req.seq,
                                error: format!("unknown plugin: {name}"),
                            };
                            let body = rmp_serde::to_vec(&()).unwrap_or_default();
                            (resp, body)
                        }
                    }
                    Err(e) => {
                        let resp = RpcResponse {
                            service_method: req.service_method.clone(),
                            seq: req.seq,
                            error: format!("decode dispense name: {e}"),
                        };
                        let body = rmp_serde::to_vec(&()).unwrap_or_default();
                        (resp, body)
                    }
                }
            }
            _ => {
                let resp = RpcResponse {
                    service_method: req.service_method.clone(),
                    seq: req.seq,
                    error: format!("unknown method: {}", req.service_method),
                };
                let body = rmp_serde::to_vec(&()).unwrap_or_default();
                (resp, body)
            }
        }
    }

    /// Check if shutdown has been requested.
    pub fn is_shutdown(&self) -> bool {
        *self.shutdown_rx.borrow()
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
    fn rpc_server_shutdown_signal() {
        let (tx, rx) = watch::channel(false);
        assert!(!*rx.borrow());
        tx.send(true).unwrap();
        assert!(*rx.borrow());
    }
}
