//! WebAssembly plugin backend powered by Extism.
//!
//! Requires the `wasm` feature flag.

use std::path::{Path, PathBuf};

use crate::backend::PluginBackend;
use crate::error::PluginError;

/// A WebAssembly plugin loaded via the Extism runtime.
///
/// Supports plugins written in any language that compiles to `wasm32-unknown-unknown`
/// or `wasm32-wasip1` (Rust, Go, JavaScript, Python, C, Zig, etc.).
pub struct WasmBackend {
    name: String,
    plugin: extism::Plugin,
    path: Option<PathBuf>,
}

impl WasmBackend {
    /// Load a `.wasm` file as an Extism plugin.
    ///
    /// The plugin name is derived from the filename (without extension).
    /// WASI support is enabled by default.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PluginError> {
        Self::load_inner(path.as_ref(), true)
    }

    /// Load a `.wasm` file with explicit WASI toggle.
    pub fn load_with_wasi(path: impl AsRef<Path>, wasi: bool) -> Result<Self, PluginError> {
        Self::load_inner(path.as_ref(), wasi)
    }

    fn load_inner(path: &Path, wasi: bool) -> Result<Self, PluginError> {
        let wasm = extism::Wasm::file(path);
        let manifest = extism::Manifest::new([wasm]);
        let plugin = extism::Plugin::new(&manifest, [], wasi).map_err(|e| PluginError::Load {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            name,
            plugin,
            path: Some(path.to_path_buf()),
        })
    }

    /// Load from an Extism `Manifest` (supports URLs, raw bytes, etc.).
    ///
    /// The `name` parameter is required since the manifest may not have a file path.
    pub fn load_manifest(
        name: impl Into<String>,
        manifest: &extism::Manifest,
        wasi: bool,
    ) -> Result<Self, PluginError> {
        let name = name.into();
        let plugin = extism::Plugin::new(manifest, [], wasi).map_err(|e| PluginError::Load {
            path: format!("<manifest:{name}>"),
            reason: e.to_string(),
        })?;
        Ok(Self {
            name,
            plugin,
            path: None,
        })
    }
}

impl PluginBackend for WasmBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn invoke(&mut self, method: &str, input: &[u8]) -> Result<Vec<u8>, PluginError> {
        self.plugin
            .call::<&[u8], Vec<u8>>(method, input)
            .map_err(|e| PluginError::Invoke {
                message: e.to_string(),
            })
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn kind(&self) -> &'static str {
        "wasm"
    }
}
