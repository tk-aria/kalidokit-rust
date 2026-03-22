//! Build-time helpers for plugin developers.
//!
//! Provides guidance and code snippets for compiling `.proto` files with
//! source comment preservation, so the `PluginInfo` service can serve
//! method descriptions at runtime.
//!
//! Since `tonic-build` is a build dependency (not available at runtime),
//! plugin developers must call `tonic-build` directly from their own `build.rs`.
//!
//! # Step 1: Plugin's `Cargo.toml`
//!
//! ```toml
//! [build-dependencies]
//! tonic-build = "0.13"
//! ```
//!
//! # Step 2: Plugin's `build.rs`
//!
//! ```ignore
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
//!
//!     tonic_build::configure()
//!         .build_server(true)
//!         .build_client(true)
//!         .file_descriptor_set_path(out_dir.join("my_plugin_descriptor.bin"))
//!         .compile_protos(
//!             &["proto/greeter.proto"],
//!             &["proto/"],
//!         )?;
//!     Ok(())
//! }
//! ```
//!
//! # Step 3: Plugin's `main.rs`
//!
//! ```ignore
//! // Embed the descriptor set compiled in build.rs
//! const MY_DESCRIPTOR: &[u8] = include_bytes!(
//!     concat!(env!("OUT_DIR"), "/my_plugin_descriptor.bin")
//! );
//!
//! // Register with the plugin server
//! go_plugin::grpc_server::serve_grpc_with_services(
//!     listener,
//!     None,
//!     Some(MY_DESCRIPTOR),  // enables PluginInfo service
//!     |router| router.add_service(GreeterServer::new(MyGreeter)),
//! ).await?;
//! ```
//!
//! # Step 4: Host queries descriptions
//!
//! ```ignore
//! let mut info_client = PluginInfoClient::new(channel);
//! let response = info_client.describe(DescribeRequest {}).await?;
//! for service in response.into_inner().services {
//!     println!("Service: {} - {}", service.name, service.description);
//!     for method in service.methods {
//!         println!("  {}: {}", method.name, method.description);
//!     }
//! }
//! ```
