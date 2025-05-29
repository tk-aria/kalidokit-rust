//! gRPC transport client implementation.
//!
//! The host uses `GRPCClient` to connect to the plugin's gRPC server,
//! dispense plugin interfaces, and manage the connection lifecycle.
//!
//! Mirrors Go's `grpc_client.go`.

use crate::error::PluginError;
use crate::grpc_broker::GRPCBroker;
use crate::grpc_server::pb;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::transport::Channel;

/// gRPC client-side protocol implementation.
///
/// Connects to the plugin's gRPC server and provides methods to
/// dispense plugin interfaces and manage the connection.
pub struct GRPCClient {
    /// The underlying gRPC channel to the plugin server.
    channel: Channel,
    /// Controller client for shutdown.
    controller: pb::grpc_controller_client::GrpcControllerClient<Channel>,
    /// Connection broker for brokered services.
    broker: Arc<GRPCBroker>,
    /// Registered plugin factories.
    plugins: HashMap<String, Box<dyn GRPCPluginFactory>>,
}

/// Factory trait for creating client-side plugin stubs.
pub trait GRPCPluginFactory: Send + Sync {
    fn create_client(
        &self,
        broker: &GRPCBroker,
        channel: Channel,
    ) -> Result<Box<dyn Any + Send>, PluginError>;
}

impl GRPCClient {
    /// Create a new gRPC client connected to the plugin server (plain TCP).
    pub async fn connect(
        addr: &str,
        plugins: HashMap<String, Box<dyn GRPCPluginFactory>>,
    ) -> Result<Self, PluginError> {
        let channel = Channel::from_shared(format!("http://{addr}"))
            .map_err(|e| PluginError::Transport(format!("invalid address: {e}")))?
            .connect()
            .await?;

        Ok(Self::from_channel(channel, plugins))
    }

    /// Create a new gRPC client with TLS.
    pub async fn connect_tls(
        addr: &str,
        tls_config: tonic::transport::ClientTlsConfig,
        plugins: HashMap<String, Box<dyn GRPCPluginFactory>>,
    ) -> Result<Self, PluginError> {
        let channel = Channel::from_shared(format!("https://{addr}"))
            .map_err(|e| PluginError::Transport(format!("invalid address: {e}")))?
            .tls_config(tls_config)
            .map_err(|e| PluginError::Tls(format!("client TLS config: {e}")))?
            .connect()
            .await?;

        Ok(Self::from_channel(channel, plugins))
    }

    /// Connect using an existing tonic channel.
    pub fn from_channel(
        channel: Channel,
        plugins: HashMap<String, Box<dyn GRPCPluginFactory>>,
    ) -> Self {
        let controller =
            pb::grpc_controller_client::GrpcControllerClient::new(channel.clone());
        let broker = Arc::new(GRPCBroker::new(false));

        // Start broker stream in background
        let broker_clone = broker.clone();
        let channel_clone = channel.clone();
        tokio::spawn(async move {
            if let Err(e) = start_broker_stream(broker_clone, channel_clone).await {
                log::debug!("Broker stream ended: {e}");
            }
        });

        Self {
            channel,
            controller,
            broker,
            plugins,
        }
    }

    /// Dispense a plugin interface by name.
    pub fn dispense(&self, name: &str) -> Result<Box<dyn Any + Send>, PluginError> {
        let factory = self
            .plugins
            .get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;
        factory.create_client(&self.broker, self.channel.clone())
    }

    /// Ping the plugin server to check if it's alive.
    pub async fn ping(&mut self) -> Result<(), PluginError> {
        use tonic_health::pb::health_client::HealthClient;
        use tonic_health::pb::HealthCheckRequest;

        let mut health = HealthClient::new(self.channel.clone());
        let request = HealthCheckRequest {
            service: "plugin".to_string(),
        };
        health
            .check(request)
            .await
            .map_err(|e| PluginError::Transport(format!("health check failed: {e}")))?;
        Ok(())
    }

    /// Request the plugin server to shut down gracefully.
    pub async fn shutdown(&mut self) -> Result<(), PluginError> {
        self.controller
            .shutdown(pb::Empty {})
            .await
            .map_err(|e| PluginError::Transport(format!("shutdown request failed: {e}")))?;
        Ok(())
    }

    /// Get the connection broker.
    pub fn broker(&self) -> &GRPCBroker {
        &self.broker
    }

    /// Get a clone of the underlying gRPC channel.
    pub fn channel(&self) -> Channel {
        self.channel.clone()
    }
}

/// Start the broker bidirectional stream with the plugin server.
async fn start_broker_stream(
    broker: Arc<GRPCBroker>,
    channel: Channel,
) -> Result<(), PluginError> {
    let mut client = pb::grpc_broker_client::GrpcBrokerClient::new(channel);

    let (_tx, rx) = tokio::sync::mpsc::channel(16);
    let outbound = tokio_stream::wrappers::ReceiverStream::new(rx);

    let response = client
        .start_stream(outbound)
        .await
        .map_err(|e| PluginError::Broker(format!("broker stream start failed: {e}")))?;

    let mut inbound = response.into_inner();

    // Process incoming ConnInfo messages and deliver to pending accepts
    while let Ok(Some(conn_info)) = inbound.message().await {
        let id = conn_info.service_id;
        let conn = crate::grpc_broker::BrokeredConn {
            network: conn_info.network,
            address: conn_info.address,
        };
        if let Err(e) = broker.deliver(id, conn).await {
            log::debug!("Broker deliver for service {id}: {e}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dispense_unknown_plugin_returns_error() {
        let channel = Channel::from_static("http://[::1]:50051").connect_lazy();
        let client = GRPCClient::from_channel(channel, HashMap::new());
        assert!(matches!(
            client.dispense("nonexistent"),
            Err(PluginError::PluginNotFound(_))
        ));
    }
}
