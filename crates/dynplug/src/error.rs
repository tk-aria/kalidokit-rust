//! Error types for dynplug.

/// Errors that can occur during plugin loading and invocation.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// Failed to load library file
    #[error("failed to load library '{path}': {reason}")]
    Load { path: String, reason: String },

    /// Symbol not found in library
    #[error("symbol not found: '{symbol}' in '{path}'")]
    SymbolNotFound { symbol: String, path: String },

    /// Interface version mismatch between host and plugin
    #[error("interface version mismatch: host expects {host}, plugin has {plugin} (library: {path})")]
    VersionMismatch { host: u32, plugin: u32, path: String },

    /// Plugin entry function returned null vtable pointer
    #[error("plugin entry returned null vtable (library: {path})")]
    NullVTable { path: String },

    /// Plugin not found in PluginManager
    #[error("plugin not found: '{0}'")]
    NotFound(String),

    /// Plugin invoke returned application error (rc = -1)
    #[error("plugin invoke error: {message}")]
    Invoke { message: String },

    /// Plugin panicked during invoke (rc = -2)
    #[error("plugin panicked during invoke")]
    Panic,

    /// Duplicate plugin name in PluginManager
    #[error("plugin '{0}' is already loaded")]
    DuplicateName(String),

    /// I/O error (directory scan, etc.)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
