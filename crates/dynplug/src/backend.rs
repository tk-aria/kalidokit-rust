//! Plugin backend abstraction.
//!
//! Provides a common trait for both native (cdylib) and WebAssembly plugins.

use std::path::Path;

use crate::error::PluginError;

/// Unified interface for plugin backends.
///
/// Both native cdylib plugins and WebAssembly (Extism) plugins implement this trait,
/// allowing `PluginManager` to handle them uniformly.
pub trait PluginBackend: std::any::Any {
    /// Upcast to `Any` for downcasting.
    fn as_any(&self) -> &dyn std::any::Any;
    /// Returns the plugin name.
    fn name(&self) -> &str;

    /// Invoke a method on the plugin.
    ///
    /// # Arguments
    /// - `method`: Method name (UTF-8)
    /// - `input`: Input data bytes
    ///
    /// # Returns
    /// Output data bytes on success.
    fn invoke(&mut self, method: &str, input: &[u8]) -> Result<Vec<u8>, PluginError>;

    /// Returns the file path this plugin was loaded from, if known.
    fn path(&self) -> Option<&Path>;

    /// Returns the plugin kind identifier (e.g., "native" or "wasm").
    fn kind(&self) -> &'static str;
}
