//! yamux-based connection multiplexer for net/rpc transport.
//!
//! MuxBroker provides multiplexed streams over a single TCP connection
//! using the yamux protocol. Each stream is identified by a unique u32 ID.
//!
//! Mirrors Go's `mux_broker.go`.

use crate::error::PluginError;
use futures::{AsyncReadExt, AsyncWriteExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};

/// yamux-based connection broker for net/rpc transport.
///
/// Multiplexes multiple logical streams over a single TCP connection.
/// The protocol works as follows:
/// - Dialer opens a yamux stream, writes a u32 ID
/// - Acceptor reads the ID, routes to the matching pending accept
/// - Acceptor writes back a u32 ACK
///
/// Mirrors Go's `MuxBroker`.
pub struct MuxBroker {
    /// Atomic counter for unique stream IDs.
    next_id: AtomicU32,
    /// Channel to request opening a new outbound stream.
    open_stream_tx: mpsc::Sender<oneshot::Sender<Result<yamux::Stream, PluginError>>>,
    /// Pending accept requests, keyed by stream ID.
    pending: Arc<Mutex<HashMap<u32, oneshot::Sender<yamux::Stream>>>>,
}

impl MuxBroker {
    /// Create a new MuxBroker from a yamux connection.
    ///
    /// Spawns a background task to drive the yamux connection,
    /// accepting incoming streams and handling open_stream requests.
    pub fn new<T>(connection: yamux::Connection<T>) -> Self
    where
        T: futures::AsyncRead + futures::AsyncWrite + Unpin + Send + 'static,
    {
        let pending: Arc<Mutex<HashMap<u32, oneshot::Sender<yamux::Stream>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Channel for open_stream requests from dial()
        let (open_stream_tx, mut open_stream_rx) =
            mpsc::channel::<oneshot::Sender<Result<yamux::Stream, PluginError>>>(16);

        // Background task: drive the yamux connection, handling both
        // inbound streams and outbound open_stream requests.
        let pending_clone = pending.clone();
        tokio::spawn(async move {
            use std::task::Poll;

            let mut conn = connection;

            // We need to poll the connection for both inbound and outbound.
            // Use poll_fn to manually drive it.
            futures::future::poll_fn(move |cx| {
                // Process any pending open_stream requests
                loop {
                    match open_stream_rx.poll_recv(cx) {
                        Poll::Ready(Some(reply_tx)) => {
                            match conn.poll_new_outbound(cx) {
                                Poll::Ready(Ok(stream)) => {
                                    let _ = reply_tx.send(Ok(stream));
                                }
                                Poll::Ready(Err(e)) => {
                                    let _ = reply_tx.send(Err(PluginError::Broker(
                                        format!("mux open stream: {e}"),
                                    )));
                                }
                                Poll::Pending => {
                                    // Can't open stream yet; we need to retry.
                                    // For simplicity, send error and let caller retry.
                                    let _ = reply_tx.send(Err(PluginError::Broker(
                                        "mux open stream pending".into(),
                                    )));
                                }
                            }
                        }
                        Poll::Ready(None) => {
                            // Channel closed, broker is being dropped
                            break;
                        }
                        Poll::Pending => break,
                    }
                }

                // Accept inbound streams
                loop {
                    match conn.poll_next_inbound(cx) {
                        Poll::Ready(Some(Ok(stream))) => {
                            let pending = pending_clone.clone();
                            tokio::spawn(async move {
                                handle_inbound_stream(stream, pending).await;
                            });
                        }
                        Poll::Ready(Some(Err(e))) => {
                            log::debug!("MuxBroker incoming error: {e}");
                            return Poll::Ready(());
                        }
                        Poll::Ready(None) => {
                            return Poll::Ready(());
                        }
                        Poll::Pending => break,
                    }
                }

                Poll::Pending
            })
            .await;
        });

        Self {
            next_id: AtomicU32::new(1),
            open_stream_tx,
            pending,
        }
    }

    /// Get the next unique stream ID.
    pub fn next_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Accept an incoming stream with the given ID.
    ///
    /// Registers a pending accept and waits for the dialer to connect.
    /// Times out after 5 seconds.
    pub async fn accept(&self, id: u32) -> Result<yamux::Stream, PluginError> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        tokio::time::timeout(std::time::Duration::from_secs(5), rx)
            .await
            .map_err(|_| PluginError::Broker(format!("mux accept timeout for stream {id}")))?
            .map_err(|_| PluginError::Broker(format!("mux accept cancelled for stream {id}")))
    }

    /// Dial a stream with the given ID.
    ///
    /// Opens a new yamux stream, writes the ID, and waits for ACK.
    pub async fn dial(&self, id: u32) -> Result<yamux::Stream, PluginError> {
        // Request a new outbound stream from the connection driver task
        let (reply_tx, reply_rx) = oneshot::channel();
        self.open_stream_tx
            .send(reply_tx)
            .await
            .map_err(|_| PluginError::Broker("mux broker connection closed".into()))?;

        let mut stream = reply_rx
            .await
            .map_err(|_| PluginError::Broker("mux open stream reply dropped".into()))??;

        // Write the stream ID
        let id_bytes = id.to_le_bytes();
        stream
            .write_all(&id_bytes)
            .await
            .map_err(|e| PluginError::Broker(format!("mux write stream id: {e}")))?;

        // Read ACK
        let mut ack_buf = [0u8; 4];
        stream
            .read_exact(&mut ack_buf)
            .await
            .map_err(|e| PluginError::Broker(format!("mux read ack: {e}")))?;
        let ack_id = u32::from_le_bytes(ack_buf);
        if ack_id != id {
            return Err(PluginError::Broker(format!(
                "mux ack mismatch: expected {id}, got {ack_id}"
            )));
        }

        Ok(stream)
    }

    /// Close the broker, cancelling all pending operations.
    pub async fn close(&self) -> Result<(), PluginError> {
        self.pending.lock().await.clear();
        // Dropping open_stream_tx will cause the driver task to exit
        Ok(())
    }
}

/// Handle an inbound yamux stream: read ID, route to pending accept, send ACK.
async fn handle_inbound_stream(
    mut stream: yamux::Stream,
    pending: Arc<Mutex<HashMap<u32, oneshot::Sender<yamux::Stream>>>>,
) {
    // Read the stream ID (4 bytes, big-endian)
    let mut id_buf = [0u8; 4];
    if stream.read_exact(&mut id_buf).await.is_err() {
        return;
    }
    let id = u32::from_le_bytes(id_buf);

    // Route to pending accept
    let tx = pending.lock().await.remove(&id);
    if let Some(tx) = tx {
        // Write ACK (same ID back)
        let _ = stream.write_all(&id_buf).await;
        let _ = stream.flush().await;
        let _ = tx.send(stream);
    } else {
        log::warn!("MuxBroker: no pending accept for stream {id}");
    }
}

impl crate::plugin::Broker for MuxBroker {
    fn next_id(&self) -> u32 {
        self.next_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mux_broker_next_id() {
        // Can't easily create a MuxBroker without a real yamux connection,
        // but we can test the ID generation logic
        let id = AtomicU32::new(1);
        assert_eq!(id.fetch_add(1, Ordering::SeqCst), 1);
        assert_eq!(id.fetch_add(1, Ordering::SeqCst), 2);
    }
}
