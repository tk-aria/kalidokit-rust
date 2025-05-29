//! End-to-end integration tests for go-plugin gRPC transport.
//!
//! Tests all 4 gRPC streaming patterns:
//! 1. Unary (request-response)
//! 2. Server streaming
//! 3. Client streaming
//! 4. Bidirectional streaming
//!
//! Also tests:
//! - Health check (ping)
//! - Graceful shutdown (controller)
//! - PluginInfo service (.proto comment extraction)

mod example_pb {
    tonic::include_proto!("example");
}

use example_pb::greeter_server::{Greeter, GreeterServer};
use example_pb::{HelloReply, HelloRequest};
use go_plugin::grpc_server::pb;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::{Request, Response, Status, Streaming};

// ---------------------------------------------------------------------------
// Greeter service implementation (plugin side)
// ---------------------------------------------------------------------------

struct GreeterImpl;

#[tonic::async_trait]
impl Greeter for GreeterImpl {
    /// Unary: single request → single response.
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        let name = request.into_inner().name;
        Ok(Response::new(HelloReply {
            message: format!("Hello, {name}!"),
        }))
    }

    type ServerStreamGreetStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<HelloReply, Status>> + Send>>;

    /// Server streaming: single request → stream of responses.
    async fn server_stream_greet(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<Self::ServerStreamGreetStream>, Status> {
        let name = request.into_inner().name;
        let greetings = vec![
            format!("Hello, {name}!"),
            format!("Bonjour, {name}!"),
            format!("こんにちは、{name}!"),
        ];

        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            for msg in greetings {
                let _ = tx.send(Ok(HelloReply { message: msg })).await;
            }
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    /// Client streaming: stream of requests → single response.
    async fn client_stream_names(
        &self,
        request: Request<Streaming<HelloRequest>>,
    ) -> Result<Response<HelloReply>, Status> {
        let mut stream = request.into_inner();
        let mut names = Vec::new();

        while let Some(req) = stream.next().await {
            let req = req?;
            names.push(req.name);
        }

        Ok(Response::new(HelloReply {
            message: format!("Hello, {}!", names.join(" & ")),
        }))
    }

    type BidiChatStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<HelloReply, Status>> + Send>>;

    /// Bidirectional streaming: stream of requests ↔ stream of responses.
    async fn bidi_chat(
        &self,
        request: Request<Streaming<HelloRequest>>,
    ) -> Result<Response<Self::BidiChatStream>, Status> {
        let mut stream = request.into_inner();
        let (tx, rx) = mpsc::channel(16);

        tokio::spawn(async move {
            while let Some(Ok(req)) = stream.next().await {
                let reply = HelloReply {
                    message: format!("Hi, {}!", req.name),
                };
                if tx.send(Ok(reply)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }
}

// ---------------------------------------------------------------------------
// Helper: start server and return address
// ---------------------------------------------------------------------------

const EXAMPLE_DESCRIPTOR: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/example_descriptor.bin"));

async fn start_test_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Build server manually to avoid capture_stdio (which breaks test output).
    tokio::spawn(async move {
        let (health_reporter, health_service) = tonic_health::server::health_reporter();
        health_reporter
            .set_service_status("plugin", tonic_health::ServingStatus::Serving)
            .await;

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let controller = go_plugin::grpc_server::ControllerService::new(shutdown_tx);

        let broker = std::sync::Arc::new(go_plugin::grpc_broker::GRPCBroker::new(false));
        let broker_service = go_plugin::grpc_broker::GRPCBrokerService::new(broker.clone());

        let stdio_server = go_plugin::grpc_stdio::GRPCStdioServer::from_readers(
            tokio::io::empty(),
            tokio::io::empty(),
        );

        let info_service =
            go_plugin::PluginInfoService::from_descriptor_bytes(EXAMPLE_DESCRIPTOR).unwrap();

        let router = tonic::transport::Server::builder()
            .add_service(health_service)
            .add_service(pb::grpc_controller_server::GrpcControllerServer::new(controller))
            .add_service(pb::grpc_broker_server::GrpcBrokerServer::new(broker_service))
            .add_service(pb::grpc_stdio_server::GrpcStdioServer::new(stdio_server))
            .add_service(pb::plugin_info_server::PluginInfoServer::new(info_service))
            .add_service(GreeterServer::new(GreeterImpl));

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
            .unwrap();
    });

    // Wait for server to be ready
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    format!("http://{addr}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_unary_say_hello() {
    let addr = start_test_server().await;
    let mut client = example_pb::greeter_client::GreeterClient::connect(addr)
        .await
        .unwrap();

    let response = client
        .say_hello(HelloRequest {
            name: "World".into(),
        })
        .await
        .unwrap();

    assert_eq!(response.into_inner().message, "Hello, World!");
}

#[tokio::test]
async fn test_server_streaming() {
    let addr = start_test_server().await;
    let mut client = example_pb::greeter_client::GreeterClient::connect(addr)
        .await
        .unwrap();

    let response = client
        .server_stream_greet(HelloRequest {
            name: "Alice".into(),
        })
        .await
        .unwrap();

    let mut stream = response.into_inner();
    let mut messages = Vec::new();
    while let Some(reply) = stream.next().await {
        messages.push(reply.unwrap().message);
    }

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0], "Hello, Alice!");
    assert_eq!(messages[1], "Bonjour, Alice!");
    assert_eq!(messages[2], "こんにちは、Alice!");
}

#[tokio::test]
async fn test_client_streaming() {
    let addr = start_test_server().await;
    let mut client = example_pb::greeter_client::GreeterClient::connect(addr)
        .await
        .unwrap();

    let names = vec!["Alice", "Bob", "Charlie"];
    let request_stream = tokio_stream::iter(
        names
            .into_iter()
            .map(|name| HelloRequest { name: name.into() }),
    );

    let response = client.client_stream_names(request_stream).await.unwrap();
    assert_eq!(
        response.into_inner().message,
        "Hello, Alice & Bob & Charlie!"
    );
}

#[tokio::test]
async fn test_bidirectional_streaming() {
    let addr = start_test_server().await;
    let mut client = example_pb::greeter_client::GreeterClient::connect(addr)
        .await
        .unwrap();

    let (tx, rx) = mpsc::channel(16);
    let request_stream = ReceiverStream::new(rx);

    let response = client.bidi_chat(request_stream).await.unwrap();
    let mut reply_stream = response.into_inner();

    // Send messages and receive replies interleaved
    tx.send(HelloRequest {
        name: "Alice".into(),
    })
    .await
    .unwrap();
    let reply = reply_stream.next().await.unwrap().unwrap();
    assert_eq!(reply.message, "Hi, Alice!");

    tx.send(HelloRequest {
        name: "Bob".into(),
    })
    .await
    .unwrap();
    let reply = reply_stream.next().await.unwrap().unwrap();
    assert_eq!(reply.message, "Hi, Bob!");

    tx.send(HelloRequest {
        name: "Charlie".into(),
    })
    .await
    .unwrap();
    let reply = reply_stream.next().await.unwrap().unwrap();
    assert_eq!(reply.message, "Hi, Charlie!");

    // Close the sender, stream should end
    drop(tx);
    assert!(reply_stream.next().await.is_none());
}

#[tokio::test]
async fn test_health_check_ping() {
    let addr = start_test_server().await;
    let channel = tonic::transport::Channel::from_shared(addr)
        .unwrap()
        .connect()
        .await
        .unwrap();

    let mut health = tonic_health::pb::health_client::HealthClient::new(channel);
    let response = health
        .check(tonic_health::pb::HealthCheckRequest {
            service: "plugin".into(),
        })
        .await
        .unwrap();

    let status = response.into_inner().status;
    // 1 = SERVING
    assert_eq!(status, 1, "expected SERVING (1), got {status}");
}

#[tokio::test]
async fn test_controller_shutdown() {
    let addr = start_test_server().await;
    let channel = tonic::transport::Channel::from_shared(addr)
        .unwrap()
        .connect()
        .await
        .unwrap();

    let mut controller =
        pb::grpc_controller_client::GrpcControllerClient::new(channel);

    let response = controller.shutdown(pb::Empty {}).await.unwrap();
    // Should return Empty without error
    let _ = response.into_inner();
}

#[tokio::test]
async fn test_plugin_info_describe() {
    let addr = start_test_server().await;
    let channel = tonic::transport::Channel::from_shared(addr)
        .unwrap()
        .connect()
        .await
        .unwrap();

    let mut info_client = pb::plugin_info_client::PluginInfoClient::new(channel);

    let response = info_client
        .describe(pb::DescribeRequest {})
        .await
        .unwrap();

    let desc = response.into_inner();
    let services = &desc.services;

    // Should contain the Greeter service (user service, not internal)
    let greeter = services
        .iter()
        .find(|s| s.name == "example.Greeter")
        .expect("Greeter service not found in PluginInfo");

    // Service should have a description from .proto comments
    assert!(
        !greeter.description.is_empty(),
        "Greeter description should not be empty, got: {:?}",
        greeter.description
    );

    // Should have 4 methods
    assert_eq!(greeter.methods.len(), 4, "Expected 4 methods");

    // Verify each method and its streaming flags
    let say_hello = greeter.methods.iter().find(|m| m.name == "SayHello").unwrap();
    assert!(!say_hello.description.is_empty(), "SayHello desc: {:?}", say_hello.description);
    assert!(!say_hello.client_streaming);
    assert!(!say_hello.server_streaming);

    let server_stream = greeter
        .methods
        .iter()
        .find(|m| m.name == "ServerStreamGreet")
        .unwrap();
    assert!(!server_stream.description.is_empty());
    assert!(!server_stream.client_streaming);
    assert!(server_stream.server_streaming);

    let client_stream = greeter
        .methods
        .iter()
        .find(|m| m.name == "ClientStreamNames")
        .unwrap();
    assert!(!client_stream.description.is_empty());
    assert!(client_stream.client_streaming);
    assert!(!client_stream.server_streaming);

    let bidi = greeter.methods.iter().find(|m| m.name == "BidiChat").unwrap();
    assert!(!bidi.description.is_empty());
    assert!(bidi.client_streaming);
    assert!(bidi.server_streaming);

    // Print descriptions for verification
    println!("=== PluginInfo Describe ===");
    println!("Service: {} - {}", greeter.name, greeter.description);
    for method in &greeter.methods {
        println!(
            "  {} [client_stream={}, server_stream={}]: {}",
            method.name, method.client_streaming, method.server_streaming, method.description
        );
    }
}
