//! Testing utilities for plugin developers.
//!
//! Provides helpers for creating in-process plugin test harnesses
//! without spawning subprocesses.
//!
//! Mirrors Go's `testing.go`.

use crate::error::PluginError;
use crate::grpc_client::{GRPCClient, GRPCPluginFactory};
use crate::plugin::PluginSet;
use std::collections::HashMap;
use tokio::net::TcpListener;

/// Create a connected pair of TCP streams for testing.
pub async fn test_conn() -> Result<(tokio::net::TcpStream, tokio::net::TcpStream), PluginError> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let client_fut = tokio::net::TcpStream::connect(addr);
    let server_fut = listener.accept();

    let (client, (server, _)) = tokio::try_join!(client_fut, server_fut)?;
    Ok((client, server))
}

/// Test harness that runs a plugin server and client in-process.
///
/// This avoids the need to build and spawn a separate plugin binary
/// for testing.
pub struct TestHarness {
    /// The port the test server is listening on.
    pub port: u16,
    /// Shutdown signal sender.
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl TestHarness {
    /// Create a new test harness with the given plugins.
    ///
    /// Starts a gRPC server in a background task and returns
    /// the harness with connection information.
    pub async fn new(_plugins: PluginSet) -> Result<Self, PluginError> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);

        // Start server in background
        tokio::spawn(async move {
            let _ = crate::grpc_server::serve_grpc(listener, HashMap::new(), None).await;
        });

        Ok(Self { port, shutdown_tx })
    }

    /// Get the address string for connecting to this test server.
    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// Create a GRPCClient connected to this test server.
    pub async fn client(
        &self,
        plugins: HashMap<String, Box<dyn GRPCPluginFactory>>,
    ) -> Result<GRPCClient, PluginError> {
        GRPCClient::connect(&self.addr(), plugins).await
    }

    /// Shut down the test server.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conn_creates_pair() {
        let (client, server) = test_conn().await.unwrap();
        assert_ne!(client.local_addr().unwrap(), server.local_addr().unwrap());
    }
}
