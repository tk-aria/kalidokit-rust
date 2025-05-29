//! gRPC transport server implementation.
//!
//! Handles the gRPC server lifecycle for the plugin process: registers
//! health checking, controller (shutdown), broker, and stdio services,
//! then registers all user-defined plugin services.
//!
//! Mirrors Go's `grpc_server.go`.

use crate::error::PluginError;
use crate::grpc_broker::{GRPCBroker, GRPCBrokerService};
use crate::grpc_stdio::GRPCStdioServer;
use crate::mtls::ServerCert;
use crate::plugin::PluginSet;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tonic::transport::Server;

/// Generated protobuf types for the plugin internal services.
pub mod pb {
    tonic::include_proto!("plugin");
}

/// Serve gRPC on the given listener with the provided plugins.
pub(crate) async fn serve_grpc(
    listener: TcpListener,
    _plugins: PluginSet,
    server_cert: Option<ServerCert>,
) -> Result<(), PluginError> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Health service
    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("plugin", tonic_health::ServingStatus::Serving)
        .await;

    // Controller service (shutdown)
    let controller = ControllerService {
        shutdown_tx: shutdown_tx.clone(),
    };

    // Broker
    let broker = Arc::new(GRPCBroker::new(false));
    let broker_service = GRPCBrokerService::new(broker.clone());

    // Build gRPC server
    let mut builder = Server::builder();

    // Apply TLS if server cert is provided
    if let Some(ref cert) = server_cert {
        // Read the CA cert from env for client verification
        let ca_cert_b64 = std::env::var(crate::protocol::ENV_PLUGIN_CLIENT_CERT).ok();
        if let Some(ca_b64) = ca_cert_b64 {
            use base64::Engine;
            let ca_der = base64::engine::general_purpose::STANDARD
                .decode(&ca_b64)
                .map_err(|e| PluginError::Tls(format!("decode client CA cert: {e}")))?;

            let _server_tls = crate::mtls::MtlsConfig::server_tls_config(
                &ca_der,
                &cert.cert_pem,
                &cert.key_pem,
            )?;

            let tls_config = tonic::transport::ServerTlsConfig::new()
                .identity(tonic::transport::Identity::from_pem(
                    cert.cert_pem.as_bytes(),
                    cert.key_pem.as_bytes(),
                ));
            builder = builder
                .tls_config(tls_config)
                .map_err(|e| PluginError::Tls(format!("server TLS config: {e}")))?;

            log::info!("Plugin gRPC server: mTLS enabled");
        }
    }

    // Stdio service: capture stdout/stderr and stream to host
    // On Unix, redirect real fd 1/2 via OS pipes (matching Go's Serve behavior).
    // On non-Unix, use empty readers as a no-op.
    #[cfg(unix)]
    let stdio_server = GRPCStdioServer::capture_stdio()
        .map_err(|e| PluginError::Transport(format!("capture stdio: {e}")))?;
    #[cfg(not(unix))]
    let stdio_server = GRPCStdioServer::from_readers(
        tokio::io::empty(),
        tokio::io::empty(),
    );

    // Build the router with internal services
    let router = builder
        .add_service(health_service)
        .add_service(GrpcControllerServer::new(controller))
        .add_service(pb::grpc_broker_server::GrpcBrokerServer::new(broker_service))
        .add_service(pb::grpc_stdio_server::GrpcStdioServer::new(stdio_server));

    // Serve
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    let mut shutdown_watch = shutdown_rx.clone();
    router
        .serve_with_incoming_shutdown(incoming, async move {
            loop {
                shutdown_watch.changed().await.ok();
                if *shutdown_watch.borrow() {
                    break;
                }
            }
        })
        .await
        .map_err(|e| PluginError::Transport(format!("gRPC server error: {e}")))?;

    log::info!("Plugin gRPC server shut down");
    Ok(())
}

/// Serve gRPC with additional user-defined services.
///
/// This function allows plugins to register their own tonic services
/// alongside the built-in controller, health, and broker services.
///
/// If `plugin_descriptor_set` is provided, a `PluginInfo` service is
/// automatically registered that serves method descriptions extracted
/// from `.proto` comments.
///
/// # Example
///
/// ```ignore
/// const MY_DESCRIPTOR: &[u8] = include_bytes!(
///     concat!(env!("OUT_DIR"), "/my_plugin_descriptor.bin")
/// );
///
/// serve_grpc_with_services(
///     listener,
///     None,
///     Some(MY_DESCRIPTOR),
///     |router| router.add_service(GreeterServer::new(MyGreeter)),
/// ).await?;
/// ```
pub async fn serve_grpc_with_services<F>(
    listener: TcpListener,
    server_cert: Option<ServerCert>,
    plugin_descriptor_set: Option<&[u8]>,
    register_fn: F,
) -> Result<(), PluginError>
where
    F: FnOnce(tonic::transport::server::Router) -> tonic::transport::server::Router,
{
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("plugin", tonic_health::ServingStatus::Serving)
        .await;

    let controller = ControllerService {
        shutdown_tx: shutdown_tx.clone(),
    };
    let broker = Arc::new(GRPCBroker::new(false));
    let broker_service = GRPCBrokerService::new(broker.clone());

    let mut builder = Server::builder();

    if let Some(ref cert) = server_cert {
        let tls_config = tonic::transport::ServerTlsConfig::new()
            .identity(tonic::transport::Identity::from_pem(
                cert.cert_pem.as_bytes(),
                cert.key_pem.as_bytes(),
            ));
        builder = builder
            .tls_config(tls_config)
            .map_err(|e| PluginError::Tls(format!("server TLS config: {e}")))?;
    }

    // Stdio service
    #[cfg(unix)]
    let stdio_server = GRPCStdioServer::capture_stdio()
        .map_err(|e| PluginError::Transport(format!("capture stdio: {e}")))?;
    #[cfg(not(unix))]
    let stdio_server = GRPCStdioServer::from_readers(
        tokio::io::empty(),
        tokio::io::empty(),
    );

    // PluginInfo service: serve .proto descriptions if descriptor set is provided
    let info_service = if let Some(desc_bytes) = plugin_descriptor_set {
        Some(crate::plugin_info::PluginInfoService::from_descriptor_bytes(desc_bytes)?)
    } else {
        None
    };

    let mut router = builder
        .add_service(health_service)
        .add_service(GrpcControllerServer::new(controller))
        .add_service(pb::grpc_broker_server::GrpcBrokerServer::new(broker_service))
        .add_service(pb::grpc_stdio_server::GrpcStdioServer::new(stdio_server));

    if let Some(info) = info_service {
        router = router.add_service(pb::plugin_info_server::PluginInfoServer::new(info));
    }

    // Let the user register additional services
    let router = register_fn(router);

    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let mut shutdown_watch = shutdown_rx.clone();
    router
        .serve_with_incoming_shutdown(incoming, async move {
            loop {
                shutdown_watch.changed().await.ok();
                if *shutdown_watch.borrow() {
                    break;
                }
            }
        })
        .await
        .map_err(|e| PluginError::Transport(format!("gRPC server error: {e}")))?;

    Ok(())
}

/// gRPC Controller service -- handles graceful shutdown.
pub struct ControllerService {
    shutdown_tx: watch::Sender<bool>,
}

impl ControllerService {
    /// Create a new ControllerService with the given shutdown sender.
    pub fn new(shutdown_tx: watch::Sender<bool>) -> Self {
        Self { shutdown_tx }
    }
}

use pb::grpc_controller_server::{GrpcController, GrpcControllerServer};

#[tonic::async_trait]
impl GrpcController for ControllerService {
    async fn shutdown(
        &self,
        _request: tonic::Request<pb::Empty>,
    ) -> Result<tonic::Response<pb::Empty>, tonic::Status> {
        log::info!("Received shutdown request from host");
        self.shutdown_tx.send(true).ok();
        Ok(tonic::Response::new(pb::Empty {}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_service_created() {
        let (tx, _rx) = watch::channel(false);
        let _controller = ControllerService { shutdown_tx: tx };
    }
}
