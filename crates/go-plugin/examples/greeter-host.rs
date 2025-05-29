//! Example host binary: spawns greeter-plugin and exercises all gRPC patterns.
//!
//! Demonstrates the full go-plugin lifecycle:
//! 1. Spawn plugin subprocess via Client
//! 2. Protocol negotiation (magic cookie + negotiation line)
//! 3. gRPC connection (health check, controller, broker, stdio)
//! 4. User service calls (unary, server streaming, client streaming, bidi)
//! 5. PluginInfo query (runtime .proto description extraction)
//! 6. Graceful shutdown

mod example_pb {
    tonic::include_proto!("example");
}

use example_pb::HelloRequest;
use go_plugin::grpc_server::pb;
use tokio_stream::StreamExt;

const MAGIC_COOKIE_KEY: &str = "GREETER_PLUGIN";
const MAGIC_COOKIE_VALUE: &str = "hello_greeter";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Locate the plugin binary (built by cargo as an example)
    let plugin_bin = std::env::current_exe()?
        .parent()
        .unwrap()
        .join("greeter-plugin");

    if !plugin_bin.exists() {
        eprintln!("Plugin binary not found at {:?}", plugin_bin);
        eprintln!("Build it first: cargo build -p go-plugin --example greeter-plugin");
        std::process::exit(1);
    }

    println!("=== go-plugin Host-Plugin Example ===");
    println!("Plugin binary: {}", plugin_bin.display());

    // --- 1. Spawn plugin ---
    let config = go_plugin::ClientConfig {
        handshake: go_plugin::HandshakeConfig {
            protocol_version: 1,
            magic_cookie_key: MAGIC_COOKIE_KEY.into(),
            magic_cookie_value: MAGIC_COOKIE_VALUE.into(),
        },
        cmd: Some(go_plugin::CmdRunner::new(&plugin_bin)),
        ..Default::default()
    };

    let mut client = go_plugin::Client::new(config);
    client.start().await?;
    println!("[host] Plugin started (addr={:?})", client.address());

    // --- 2. Health check (ping) ---
    client.ping().await?;
    println!("[host] Health check: OK");

    // Get the raw gRPC channel for user service calls
    let channel = client
        .grpc_client()
        .expect("grpc client not available")
        .channel();

    // --- 3. Unary RPC ---
    {
        let mut greeter = example_pb::greeter_client::GreeterClient::new(channel.clone());
        let resp = greeter
            .say_hello(HelloRequest {
                name: "World".into(),
            })
            .await?;
        let msg = resp.into_inner().message;
        assert_eq!(msg, "Hello, World!");
        println!("[host] Unary SayHello: {msg}");
    }

    // --- 4. Server Streaming ---
    {
        let mut greeter = example_pb::greeter_client::GreeterClient::new(channel.clone());
        let resp = greeter
            .server_stream_greet(HelloRequest {
                name: "Alice".into(),
            })
            .await?;
        let mut stream = resp.into_inner();
        let mut messages = Vec::new();
        while let Some(reply) = stream.next().await {
            messages.push(reply?.message);
        }
        assert_eq!(messages.len(), 3);
        println!("[host] Server streaming: received {} messages", messages.len());
        for m in &messages {
            println!("  - {m}");
        }
    }

    // --- 5. Client Streaming ---
    {
        let mut greeter = example_pb::greeter_client::GreeterClient::new(channel.clone());
        let request_stream = tokio_stream::iter(
            ["Alice", "Bob", "Charlie"]
                .into_iter()
                .map(|name| HelloRequest { name: name.to_string() }),
        );
        let resp = greeter.client_stream_names(request_stream).await?;
        let msg = resp.into_inner().message;
        assert_eq!(msg, "Hello, Alice & Bob & Charlie!");
        println!("[host] Client streaming: {msg}");
    }

    // --- 6. Bidirectional Streaming ---
    {
        let mut greeter = example_pb::greeter_client::GreeterClient::new(channel.clone());
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let request_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

        let resp = greeter.bidi_chat(request_stream).await?;
        let mut reply_stream = resp.into_inner();

        for name in &["Alice", "Bob", "Charlie"] {
            tx.send(HelloRequest {
                name: name.to_string(),
            })
            .await?;
            let reply = reply_stream.next().await.unwrap()?;
            assert_eq!(reply.message, format!("Hi, {name}!"));
            println!("[host] Bidi: sent {name} → {}", reply.message);
        }
        drop(tx);
        assert!(reply_stream.next().await.is_none());
        println!("[host] Bidi streaming: OK (stream closed cleanly)");
    }

    // --- 7. PluginInfo Describe ---
    {
        let mut info_client = pb::plugin_info_client::PluginInfoClient::new(channel.clone());
        let resp = info_client.describe(pb::DescribeRequest {}).await?;
        let desc = resp.into_inner();
        println!("[host] PluginInfo: {} service(s) found", desc.services.len());
        for svc in &desc.services {
            println!("  Service: {} - {}", svc.name, svc.description);
            for method in &svc.methods {
                println!(
                    "    {} [client_stream={}, server_stream={}]: {}",
                    method.name, method.client_streaming, method.server_streaming, method.description
                );
            }
        }
        let greeter = desc.services.iter().find(|s| s.name == "example.Greeter");
        assert!(greeter.is_some(), "Greeter service not found in PluginInfo");
        assert_eq!(greeter.unwrap().methods.len(), 4);
    }

    // --- 8. Graceful shutdown ---
    println!("[host] Shutting down plugin...");
    client.kill().await;
    assert!(client.exited());
    println!("[host] Plugin shut down cleanly.");

    println!("\n=== All checks passed! ===");
    Ok(())
}
