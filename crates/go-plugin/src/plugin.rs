//! Plugin interface definitions.
//!
//! Plugin authors implement these traits to define their plugin's behavior.
//! The host and plugin each get different views: the host gets a client stub,
//! the plugin provides a server implementation.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// A set of named plugins.
pub type PluginSet = HashMap<String, Arc<dyn Plugin>>;

/// Map of protocol version to plugin set.
pub type VersionedPlugins = HashMap<u32, PluginSet>;

/// Interface that plugin authors implement.
///
/// For gRPC plugins, implement `GRPCPlugin` instead (which extends this trait).
/// For net/rpc plugins, implement `Plugin` directly.
///
/// Mirrors Go's `Plugin` interface.
pub trait Plugin: Send + Sync + 'static {
    /// Create the server-side implementation for net/rpc transport.
    fn server(&self, broker: &dyn Broker) -> Option<Box<dyn Any + Send>>;

    /// Create the client-side stub for net/rpc transport.
    fn client(&self, broker: &dyn Broker, client: Box<dyn Any + Send>) -> Option<Box<dyn Any + Send>>;

    /// Upcast to GRPCPlugin if this plugin supports gRPC.
    /// Override this to return `Some(self)` in GRPCPlugin implementations.
    fn as_grpc(&self) -> Option<&dyn GRPCPlugin> {
        None
    }
}

/// Extended plugin interface for gRPC-based plugins.
///
/// Mirrors Go's `GRPCPlugin` interface.
pub trait GRPCPlugin: Plugin {
    /// Register the plugin's gRPC service on the given server.
    fn grpc_server(
        &self,
        broker: &dyn Broker,
        registrar: &mut dyn GRPCServiceRegistrar,
    ) -> Result<(), crate::error::PluginError>;

    /// Create the client-side stub wrapping the gRPC connection.
    fn grpc_client(
        &self,
        broker: &dyn Broker,
        channel: tonic::transport::Channel,
    ) -> Result<Box<dyn Any + Send>, crate::error::PluginError>;
}

/// Broker for establishing additional connections between host and plugin.
pub trait Broker: Send + Sync {
    fn next_id(&self) -> u32;
}

/// Registrar for adding gRPC services to the plugin's server.
pub trait GRPCServiceRegistrar {
    fn register_service(&mut self, service: tonic::service::Routes);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn server(&self, _broker: &dyn Broker) -> Option<Box<dyn Any + Send>> {
            None
        }
        fn client(&self, _broker: &dyn Broker, _client: Box<dyn Any + Send>) -> Option<Box<dyn Any + Send>> {
            None
        }
    }

    #[test]
    fn plugin_set_insert_and_lookup() {
        let mut set: PluginSet = HashMap::new();
        set.insert("test".to_string(), Arc::new(TestPlugin));
        assert!(set.contains_key("test"));
    }

    #[test]
    fn versioned_plugins() {
        let mut versioned: VersionedPlugins = HashMap::new();
        let mut v1: PluginSet = HashMap::new();
        v1.insert("greeter".to_string(), Arc::new(TestPlugin));
        versioned.insert(1, v1);
        assert!(versioned[&1].contains_key("greeter"));
    }
}
