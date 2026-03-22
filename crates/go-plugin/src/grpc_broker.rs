//! gRPC connection broker.
//!
//! The GRPCBroker allows plugins and hosts to establish additional
//! connections beyond the main plugin connection. This is used for:
//! - Plugin callbacks to the host
//! - Host providing additional services to the plugin
//! - Multi-service plugin architectures
//!
//! Mirrors Go's `grpc_broker.go`.

use crate::error::PluginError;
use crate::grpc_server::pb;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc, oneshot};

/// Broker for establishing additional gRPC connections between host and plugin.
///
/// Uses a bidirectional gRPC stream to exchange connection information.
/// Each brokered connection gets a unique service ID.
pub struct GRPCBroker {
    /// Atomic counter for unique service IDs.
    next_id: AtomicU32,
    /// Pending accept requests, keyed by service ID.
    pending: Arc<Mutex<HashMap<u32, oneshot::Sender<BrokeredConn>>>>,
    /// Outbound stream sender for sending ConnInfo to the peer.
    /// Set when the broker stream is started.
    stream_tx: Mutex<Option<mpsc::Sender<pb::ConnInfo>>>,
    /// Whether multiplexing is enabled.
    mux_enabled: bool,
}

/// A brokered connection with its address information.
pub struct BrokeredConn {
    /// Network type ("unix" or "tcp").
    pub network: String,
    /// Address to connect to.
    pub address: String,
}

impl GRPCBroker {
    /// Create a new broker.
    pub fn new(mux_enabled: bool) -> Self {
        Self {
            next_id: AtomicU32::new(1),
            pending: Arc::new(Mutex::new(HashMap::new())),
            stream_tx: Mutex::new(None),
            mux_enabled,
        }
    }

    /// Get the next unique service ID.
    pub fn next_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Set the outbound stream sender (called when broker stream starts).
    pub async fn set_stream_tx(&self, tx: mpsc::Sender<pb::ConnInfo>) {
        *self.stream_tx.lock().await = Some(tx);
    }

    /// Accept a brokered connection for the given service ID.
    ///
    /// Creates a listener, sends the connection info over the broker stream,
    /// and waits for the other side to connect (5 second timeout).
    pub async fn accept(
        &self,
        id: u32,
    ) -> Result<tokio::net::TcpStream, PluginError> {
        let stream_tx = self.stream_tx.lock().await;
        let tx = stream_tx.as_ref().ok_or_else(|| {
            PluginError::Broker("broker stream not started".into())
        })?;

        // Bind a new listener for this service
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| PluginError::Broker(format!("bind broker listener: {e}")))?;
        let local_addr = listener.local_addr()?;

        // Send connection info to the other side
        let conn_info = pb::ConnInfo {
            service_id: id,
            network: "tcp".to_string(),
            address: local_addr.to_string(),
            knock: None,
        };
        tx.send(conn_info).await.map_err(|e| {
            PluginError::Broker(format!("send conn info: {e}"))
        })?;

        // Accept the incoming connection (5s timeout, matching Go)
        let (stream, _addr) = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            listener.accept(),
        )
        .await
        .map_err(|_| PluginError::Broker(format!("broker accept timeout for service {id}")))?
        .map_err(|e| PluginError::Broker(format!("broker accept error: {e}")))?;

        Ok(stream)
    }

    /// Accept a brokered connection and serve a gRPC service on it.
    ///
    /// This is the primary API for plugins that need to provide additional
    /// services via brokered connections.
    ///
    /// Mirrors Go's `GRPCBroker.AcceptAndServe`.
    pub async fn accept_and_serve<F>(
        &self,
        id: u32,
        register_fn: F,
    ) -> Result<(), PluginError>
    where
        F: FnOnce(tonic::transport::Server) -> tonic::transport::server::Router,
    {
        let stream = self.accept(id).await?;
        let local_addr = stream.local_addr()?;

        // Create a new listener on the same port for tonic
        let listener = TcpListener::bind(local_addr)
            .await
            .map_err(|e| PluginError::Broker(format!("rebind for serve: {e}")))?;

        let builder = tonic::transport::Server::builder();
        let router = register_fn(builder);

        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        router
            .serve_with_incoming(incoming)
            .await
            .map_err(|e| PluginError::Broker(format!("broker serve error: {e}")))?;

        Ok(())
    }

    /// Dial a brokered connection and return a gRPC channel.
    ///
    /// Waits for the peer to send connection info for this service ID,
    /// then creates a tonic Channel to it.
    ///
    /// Mirrors Go's `GRPCBroker.Dial`.
    pub async fn dial_grpc(
        &self,
        id: u32,
    ) -> Result<tonic::transport::Channel, PluginError> {
        // Register pending and wait for conn info
        let rx = self.register_pending(id).await;

        let conn_info = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rx,
        )
        .await
        .map_err(|_| PluginError::Broker(format!("broker dial timeout for service {id}")))?
        .map_err(|_| PluginError::Broker(format!("broker pending dropped for service {id}")))?;

        let channel = tonic::transport::Channel::from_shared(format!(
            "http://{}",
            conn_info.address
        ))
        .map_err(|e| PluginError::Broker(format!("invalid broker address: {e}")))?
        .connect()
        .await
        .map_err(|e| PluginError::Broker(format!("broker dial connect: {e}")))?;

        Ok(channel)
    }

    /// Dial a brokered connection and return a raw TCP stream.
    pub async fn dial_tcp(
        &self,
        id: u32,
        conn_info: BrokeredConn,
    ) -> Result<tokio::net::TcpStream, PluginError> {
        let addr: std::net::SocketAddr = conn_info.address.parse().map_err(|e| {
            PluginError::Broker(format!("invalid broker address: {e}"))
        })?;

        let stream = tokio::net::TcpStream::connect(addr)
            .await
            .map_err(|e| PluginError::Broker(format!("broker dial for service {id}: {e}")))?;

        Ok(stream)
    }

    /// Register a pending accept for a service ID.
    pub async fn register_pending(&self, id: u32) -> oneshot::Receiver<BrokeredConn> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        rx
    }

    /// Deliver connection info for a pending accept.
    pub async fn deliver(&self, id: u32, conn: BrokeredConn) -> Result<(), PluginError> {
        let tx = self
            .pending
            .lock()
            .await
            .remove(&id)
            .ok_or_else(|| PluginError::Broker(format!("no pending accept for service {id}")))?;
        tx.send(conn)
            .map_err(|_| PluginError::Broker(format!("pending receiver dropped for service {id}")))?;
        Ok(())
    }

    /// Close the broker, cancelling all pending operations.
    pub async fn close(&self) {
        let mut pending = self.pending.lock().await;
        pending.clear();
        *self.stream_tx.lock().await = None;
    }

    /// Whether multiplexing is enabled.
    pub fn mux_enabled(&self) -> bool {
        self.mux_enabled
    }
}

impl crate::plugin::Broker for GRPCBroker {
    fn next_id(&self) -> u32 {
        self.next_id()
    }
}

/// gRPC service implementation for the broker bidirectional stream.
pub struct GRPCBrokerService {
    broker: Arc<GRPCBroker>,
}

impl GRPCBrokerService {
    pub fn new(broker: Arc<GRPCBroker>) -> Self {
        Self { broker }
    }
}

use pb::grpc_broker_server::GrpcBroker;

#[tonic::async_trait]
impl GrpcBroker for GRPCBrokerService {
    type StartStreamStream = tokio_stream::wrappers::ReceiverStream<Result<pb::ConnInfo, tonic::Status>>;

    async fn start_stream(
        &self,
        request: tonic::Request<tonic::Streaming<pb::ConnInfo>>,
    ) -> Result<tonic::Response<Self::StartStreamStream>, tonic::Status> {
        let mut inbound = request.into_inner();
        let broker = self.broker.clone();

        // Outbound channel: the broker can send ConnInfo to the peer
        let (outbound_tx, outbound_rx) = mpsc::channel(16);

        // Store the sender so accept() can use it
        {
            let broker_clone = broker.clone();
            let tx = outbound_tx.clone();
            tokio::spawn(async move {
                broker_clone.set_stream_tx(tx).await;
            });
        }

        // Process incoming connection info messages
        let broker_for_recv = broker.clone();
        tokio::spawn(async move {
            while let Ok(Some(conn_info)) = inbound.message().await {
                let id = conn_info.service_id;

                // Check if this is a knock message (multiplexing)
                if let Some(ref knock) = conn_info.knock {
                    if knock.knock {
                        log::debug!("Received broker knock for service {id}");
                        // TODO: multiplexing support
                        continue;
                    }
                }

                let conn = BrokeredConn {
                    network: conn_info.network,
                    address: conn_info.address,
                };
                if let Err(e) = broker_for_recv.deliver(id, conn).await {
                    log::debug!("Broker deliver for service {id}: {e}");
                }
            }
        });

        // Convert outbound channel to a stream of Results
        let (result_tx, result_rx) = mpsc::channel(16);
        tokio::spawn(async move {
            let mut outbound_rx = outbound_rx;
            while let Some(info) = outbound_rx.recv().await {
                if result_tx.send(Ok(info)).await.is_err() {
                    break;
                }
            }
        });

        Ok(tonic::Response::new(
            tokio_stream::wrappers::ReceiverStream::new(result_rx),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_next_id_increments() {
        let broker = GRPCBroker::new(false);
        assert_eq!(broker.next_id(), 1);
        assert_eq!(broker.next_id(), 2);
        assert_eq!(broker.next_id(), 3);
    }

    #[tokio::test]
    async fn broker_register_and_deliver() {
        let broker = GRPCBroker::new(false);
        let rx = broker.register_pending(42).await;

        broker
            .deliver(42, BrokeredConn {
                network: "tcp".into(),
                address: "127.0.0.1:5000".into(),
            })
            .await
            .unwrap();

        let conn = rx.await.unwrap();
        assert_eq!(conn.network, "tcp");
        assert_eq!(conn.address, "127.0.0.1:5000");
    }

    #[tokio::test]
    async fn broker_deliver_no_pending_errors() {
        let broker = GRPCBroker::new(false);
        let result = broker
            .deliver(99, BrokeredConn {
                network: "tcp".into(),
                address: "127.0.0.1:5000".into(),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn broker_close_clears_pending() {
        let broker = GRPCBroker::new(false);
        let _rx = broker.register_pending(1).await;
        let _rx2 = broker.register_pending(2).await;
        broker.close().await;
        assert!(broker.pending.lock().await.is_empty());
    }
}
