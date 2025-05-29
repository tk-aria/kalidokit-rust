//! Example plugin binary: Greeter service.
//!
//! This binary is spawned as a subprocess by the host (greeter-host).
//! It implements the go-plugin subprocess protocol:
//! 1. Check magic cookie (env var)
//! 2. Bind TCP listener
//! 3. Write negotiation line to stdout
//! 4. Serve gRPC with user-defined services

mod example_pb {
    tonic::include_proto!("example");
}

use example_pb::greeter_server::{Greeter, GreeterServer};
use example_pb::{HelloReply, HelloRequest};
use std::io::Write;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::{Request, Response, Status, Streaming};

const MAGIC_COOKIE_KEY: &str = "GREETER_PLUGIN";
const MAGIC_COOKIE_VALUE: &str = "hello_greeter";

const EXAMPLE_DESCRIPTOR: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/example_descriptor.bin"));

// ---------------------------------------------------------------------------
// Greeter service implementation
// ---------------------------------------------------------------------------

struct GreeterImpl;

#[tonic::async_trait]
impl Greeter for GreeterImpl {
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

    async fn client_stream_names(
        &self,
        request: Request<Streaming<HelloRequest>>,
    ) -> Result<Response<HelloReply>, Status> {
        let mut stream = request.into_inner();
        let mut names = Vec::new();
        while let Some(req) = stream.next().await {
            names.push(req?.name);
        }
        Ok(Response::new(HelloReply {
            message: format!("Hello, {}!", names.join(" & ")),
        }))
    }

    type BidiChatStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<HelloReply, Status>> + Send>>;

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
// main: go-plugin subprocess protocol
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Check magic cookie
    match std::env::var(MAGIC_COOKIE_KEY) {
        Ok(val) if val == MAGIC_COOKIE_VALUE => {}
        _ => {
            eprintln!("This binary is a go-plugin plugin. Do not run it directly.");
            eprintln!("Set {}={} to run as a plugin.", MAGIC_COOKIE_KEY, MAGIC_COOKIE_VALUE);
            std::process::exit(1);
        }
    }

    // 2. Bind TCP listener
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    // 3. Write negotiation line to stdout (core_ver|app_ver|network|address|protocol)
    let negotiation_line = format!("1|1|tcp|{addr}|grpc");
    {
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{negotiation_line}")?;
        stdout.flush()?;
    }

    // 4. Serve gRPC with Greeter service
    go_plugin::grpc_server::serve_grpc_with_services(
        listener,
        None, // no TLS
        Some(EXAMPLE_DESCRIPTOR),
        |router| router.add_service(GreeterServer::new(GreeterImpl)),
    )
    .await?;

    Ok(())
}
