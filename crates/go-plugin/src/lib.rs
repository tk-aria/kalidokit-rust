//! Rust port of HashiCorp go-plugin.
//!
//! A subprocess-based plugin system over RPC. Plugins run as separate OS
//! processes communicating with the host via gRPC (recommended) or a custom
//! net/rpc-compatible protocol over yamux.
//!
//! # Architecture
//!
//! ```text
//!     HOST PROCESS                      PLUGIN PROCESS
//! ┌──────────────────┐              ┌──────────────────┐
//! │  Client           │  subprocess  │  serve()          │
//! │  ├─ ClientConfig  │───spawn────>│  ├─ ServeConfig   │
//! │  ├─ Runner        │              │  ├─ HandshakeConfig│
//! │  └─ GRPCClient   │<──connect──>│  └─ GRPCServer    │
//! │     ├─ Broker     │<──broker──>│     ├─ Broker      │
//! │     └─ StdioClient│<──stdio───>│     └─ StdioServer │
//! └──────────────────┘              └──────────────────┘
//! ```
//!
//! # Quick Start (Host)
//!
//! ```no_run
//! use go_plugin::{client::Client, client::ClientConfig, runner::CmdRunner};
//!
//! let config = ClientConfig {
//!     cmd: Some(CmdRunner::new("./my-plugin")),
//!     ..Default::default()
//! };
//! let mut client = Client::new(config);
//! // client.start().await?;
//! // let greeter = client.dispense("greeter")?;
//! ```
//!
//! # Quick Start (Plugin)
//!
//! ```no_run
//! use go_plugin::server::{serve, ServeConfig};
//! use go_plugin::protocol::HandshakeConfig;
//!
//! let config = ServeConfig {
//!     handshake: HandshakeConfig {
//!         protocol_version: 1,
//!         magic_cookie_key: "MY_PLUGIN".into(),
//!         magic_cookie_value: "hello".into(),
//!     },
//!     plugins: Default::default(),
//!     versioned_plugins: Default::default(),
//!     grpc_server_options: vec![],
//!     test: None,
//! };
//! // serve(config).await?;
//! ```

pub mod client;
pub mod discover;
pub mod error;
pub mod grpc_broker;
pub mod grpc_client;
pub mod grpc_server;
pub mod grpc_stdio;
pub mod mtls;
pub mod mux_broker;
pub mod plugin;
pub mod plugin_build;
pub mod plugin_info;
pub mod protocol;
pub mod rpc_client;
pub mod rpc_codec;
pub mod rpc_server;
pub mod runner;
pub mod secure;
pub mod server;
pub mod testing;

// Re-export commonly used types
pub use client::{Client, ClientConfig};
pub use error::PluginError;
pub use plugin::{GRPCPlugin, Plugin};
pub use protocol::{HandshakeConfig, Protocol};
pub use runner::{CmdRunner, Runner};
pub use server::{ServeConfig, serve};
pub use plugin_info::{DescriptorRegistry, PluginInfoService};
