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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_load() {
        let e = PluginError::Load {
            path: "/tmp/test.so".to_string(),
            reason: "file not found".to_string(),
        };
        assert_eq!(
            e.to_string(),
            "failed to load library '/tmp/test.so': file not found"
        );
    }

    #[test]
    fn test_display_symbol_not_found() {
        let e = PluginError::SymbolNotFound {
            symbol: "foo".to_string(),
            path: "/tmp/test.so".to_string(),
        };
        assert_eq!(
            e.to_string(),
            "symbol not found: 'foo' in '/tmp/test.so'"
        );
    }

    #[test]
    fn test_display_version_mismatch() {
        let e = PluginError::VersionMismatch {
            host: 1,
            plugin: 2,
            path: "/tmp/test.so".to_string(),
        };
        assert!(e.to_string().contains("host expects 1"));
        assert!(e.to_string().contains("plugin has 2"));
    }

    #[test]
    fn test_display_null_vtable() {
        let e = PluginError::NullVTable {
            path: "/tmp/test.so".to_string(),
        };
        assert!(e.to_string().contains("null vtable"));
    }

    #[test]
    fn test_display_not_found() {
        let e = PluginError::NotFound("myplugin".to_string());
        assert_eq!(e.to_string(), "plugin not found: 'myplugin'");
    }

    #[test]
    fn test_display_invoke() {
        let e = PluginError::Invoke {
            message: "bad input".to_string(),
        };
        assert_eq!(e.to_string(), "plugin invoke error: bad input");
    }

    #[test]
    fn test_display_panic() {
        let e = PluginError::Panic;
        assert_eq!(e.to_string(), "plugin panicked during invoke");
    }

    #[test]
    fn test_display_duplicate_name() {
        let e = PluginError::DuplicateName("greeter".to_string());
        assert_eq!(e.to_string(), "plugin 'greeter' is already loaded");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let e: PluginError = io_err.into();
        assert!(e.to_string().contains("gone"));
        assert!(matches!(e, PluginError::Io(_)));
    }
}
