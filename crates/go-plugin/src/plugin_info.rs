//! Plugin introspection service.
//!
//! Provides runtime access to service and method descriptions extracted
//! from `.proto` file comments. The host can call `PluginInfo.Describe()`
//! to discover what RPCs a plugin offers and what each one does.
//!
//! # How it works
//!
//! 1. `tonic-build` compiles `.proto` files with `file_descriptor_set_path`,
//!    which preserves source comments in a binary descriptor set.
//! 2. The descriptor set is embedded in the binary via `include_bytes!`.
//! 3. `DescriptorRegistry` parses it and extracts leading comments for
//!    each service and method.
//! 4. `PluginInfoService` serves this data over the `PluginInfo` gRPC service.
//!
//! # For plugin developers
//!
//! In your `build.rs`, use `go_plugin::plugin_build::compile_plugin_protos()`
//! to compile your `.proto` files with source info enabled. Then register
//! your descriptor set when creating the plugin server.

use crate::grpc_server::pb;
use prost::Message;
use prost_types::{FileDescriptorProto, FileDescriptorSet, SourceCodeInfo};

/// Internal descriptor for go-plugin's own proto files.
pub const INTERNAL_FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/plugin_descriptor.bin"));

/// Registry that extracts service/method descriptions from file descriptor sets.
///
/// Parses one or more `FileDescriptorSet` binaries and builds a lookup
/// table of service descriptions with comments from `.proto` files.
pub struct DescriptorRegistry {
    services: Vec<pb::ServiceInfo>,
}

impl DescriptorRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            services: Vec::new(),
        }
    }

    /// Add a file descriptor set (compiled `.proto` with source info).
    ///
    /// Call this for each descriptor set you want to include.
    /// Internal go-plugin services are automatically excluded.
    pub fn add_descriptor_set(&mut self, bytes: &[u8]) -> Result<(), crate::error::PluginError> {
        let fds = FileDescriptorSet::decode(bytes).map_err(|e| {
            crate::error::PluginError::Other(format!("failed to decode file descriptor set: {e}"))
        })?;

        for file in &fds.file {
            self.extract_services(file);
        }

        Ok(())
    }

    /// Add a file descriptor set, but skip internal go-plugin services
    /// (GRPCBroker, GRPCController, GRPCStdio, PluginInfo).
    pub fn add_user_descriptor_set(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), crate::error::PluginError> {
        let fds = FileDescriptorSet::decode(bytes).map_err(|e| {
            crate::error::PluginError::Other(format!("failed to decode file descriptor set: {e}"))
        })?;

        for file in &fds.file {
            self.extract_services_filtered(file, true);
        }

        Ok(())
    }

    /// Get all registered service descriptions.
    pub fn services(&self) -> &[pb::ServiceInfo] {
        &self.services
    }

    fn extract_services(&mut self, file: &FileDescriptorProto) {
        self.extract_services_filtered(file, false);
    }

    fn extract_services_filtered(&mut self, file: &FileDescriptorProto, skip_internal: bool) {
        let package = file.package.as_deref().unwrap_or("");
        let source_info = file.source_code_info.as_ref();

        for (svc_idx, svc) in file.service.iter().enumerate() {
            let svc_name = svc.name.as_deref().unwrap_or("");
            let full_name = if package.is_empty() {
                svc_name.to_string()
            } else {
                format!("{package}.{svc_name}")
            };

            // Skip internal go-plugin services if requested
            if skip_internal && is_internal_service(svc_name) {
                continue;
            }

            // Service comment: path = [6, svc_idx]
            // 6 = FileDescriptorProto.service field number
            let svc_comment =
                get_leading_comment(source_info, &[6, svc_idx as i32]).unwrap_or_default();

            let mut methods = Vec::new();
            for (method_idx, method) in svc.method.iter().enumerate() {
                let method_name = method.name.as_deref().unwrap_or("");

                // Method comment: path = [6, svc_idx, 2, method_idx]
                // 2 = ServiceDescriptorProto.method field number
                let method_comment =
                    get_leading_comment(source_info, &[6, svc_idx as i32, 2, method_idx as i32])
                        .unwrap_or_default();

                methods.push(pb::MethodInfo {
                    name: method_name.to_string(),
                    description: method_comment,
                    input_type: clean_type_name(method.input_type.as_deref().unwrap_or("")),
                    output_type: clean_type_name(method.output_type.as_deref().unwrap_or("")),
                    client_streaming: method.client_streaming.unwrap_or(false),
                    server_streaming: method.server_streaming.unwrap_or(false),
                });
            }

            self.services.push(pb::ServiceInfo {
                name: full_name,
                description: svc_comment,
                methods,
            });
        }
    }
}

impl Default for DescriptorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// gRPC service implementation for PluginInfo.
///
/// Returns service and method descriptions extracted from file descriptor sets.
pub struct PluginInfoService {
    services: Vec<pb::ServiceInfo>,
}

impl PluginInfoService {
    /// Create from a populated DescriptorRegistry.
    pub fn from_registry(registry: DescriptorRegistry) -> Self {
        Self {
            services: registry.services,
        }
    }

    /// Create from raw descriptor set bytes (convenience).
    ///
    /// Parses the descriptor set and extracts user service descriptions,
    /// skipping internal go-plugin services.
    pub fn from_descriptor_bytes(bytes: &[u8]) -> Result<Self, crate::error::PluginError> {
        let mut registry = DescriptorRegistry::new();
        registry.add_user_descriptor_set(bytes)?;
        Ok(Self::from_registry(registry))
    }
}

use pb::plugin_info_server::PluginInfo;

#[tonic::async_trait]
impl PluginInfo for PluginInfoService {
    async fn describe(
        &self,
        _request: tonic::Request<pb::DescribeRequest>,
    ) -> Result<tonic::Response<pb::DescribeResponse>, tonic::Status> {
        Ok(tonic::Response::new(pb::DescribeResponse {
            services: self.services.clone(),
        }))
    }
}

/// Check if a service name is an internal go-plugin service.
fn is_internal_service(name: &str) -> bool {
    matches!(
        name,
        "GRPCBroker" | "GRPCController" | "GRPCStdio" | "PluginInfo"
    )
}

/// Extract a leading comment from source code info by path.
///
/// The `path` identifies a specific element in the `.proto` file.
/// See `google.protobuf.SourceCodeInfo` for path encoding details.
fn get_leading_comment(source_info: Option<&SourceCodeInfo>, path: &[i32]) -> Option<String> {
    let source_info = source_info?;
    for location in &source_info.location {
        if location.path == path {
            if let Some(ref comment) = location.leading_comments {
                return Some(clean_comment(comment));
            }
        }
    }
    None
}

/// Clean up a protobuf comment string.
/// Removes leading/trailing whitespace and normalizes line breaks.
fn clean_comment(comment: &str) -> String {
    comment
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Clean up a fully qualified type name (remove leading dot).
fn clean_type_name(name: &str) -> String {
    name.strip_prefix('.').unwrap_or(name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_internal_descriptor_set() {
        let mut registry = DescriptorRegistry::new();
        registry.add_descriptor_set(INTERNAL_FILE_DESCRIPTOR_SET).unwrap();
        let services = registry.services();

        // Should contain our internal services
        let names: Vec<&str> = services.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"plugin.GRPCBroker"), "services: {names:?}");
        assert!(names.contains(&"plugin.GRPCController"), "services: {names:?}");
        assert!(names.contains(&"plugin.GRPCStdio"), "services: {names:?}");
        assert!(names.contains(&"plugin.PluginInfo"), "services: {names:?}");
    }

    #[test]
    fn user_descriptor_set_skips_internal() {
        let mut registry = DescriptorRegistry::new();
        registry
            .add_user_descriptor_set(INTERNAL_FILE_DESCRIPTOR_SET)
            .unwrap();
        let services = registry.services();

        // Internal services should be excluded
        let names: Vec<&str> = services.iter().map(|s| s.name.as_str()).collect();
        assert!(!names.contains(&"plugin.GRPCBroker"));
        assert!(!names.contains(&"plugin.GRPCController"));
        assert!(!names.contains(&"plugin.GRPCStdio"));
        assert!(!names.contains(&"plugin.PluginInfo"));
    }

    #[test]
    fn stdio_service_has_description() {
        let mut registry = DescriptorRegistry::new();
        registry.add_descriptor_set(INTERNAL_FILE_DESCRIPTOR_SET).unwrap();

        let stdio = registry
            .services()
            .iter()
            .find(|s| s.name == "plugin.GRPCStdio")
            .expect("GRPCStdio service not found");

        // grpc_stdio.proto has a comment on the service
        assert!(
            !stdio.description.is_empty(),
            "GRPCStdio should have a description from proto comments"
        );

        // StreamStdio method should have a description
        let stream_method = stdio
            .methods
            .iter()
            .find(|m| m.name == "StreamStdio")
            .expect("StreamStdio method not found");
        assert!(
            !stream_method.description.is_empty(),
            "StreamStdio should have a description"
        );
    }

    #[test]
    fn clean_comment_normalizes() {
        assert_eq!(clean_comment(" hello \n world "), "hello world");
        assert_eq!(clean_comment("  single  "), "single");
        assert_eq!(clean_comment("\n\n"), "");
    }

    #[test]
    fn clean_type_name_strips_dot() {
        assert_eq!(clean_type_name(".myapp.HelloRequest"), "myapp.HelloRequest");
        assert_eq!(clean_type_name("myapp.HelloReply"), "myapp.HelloReply");
    }

    #[test]
    fn is_internal_service_check() {
        assert!(is_internal_service("GRPCBroker"));
        assert!(is_internal_service("GRPCController"));
        assert!(is_internal_service("GRPCStdio"));
        assert!(is_internal_service("PluginInfo"));
        assert!(!is_internal_service("Greeter"));
        assert!(!is_internal_service("MyService"));
    }
}
