//! Centralized plugin lifecycle management.

use std::collections::HashMap;
use std::path::Path;

use crate::api::PluginVTable;
use crate::backend::PluginBackend;
use crate::error::PluginError;
use crate::loader::LoadedLibrary;
use crate::native::NativeBackend;

/// Internal record for a managed plugin.
struct ManagedPlugin {
    backend: Box<dyn PluginBackend>,
}

/// Manages the lifecycle of multiple plugins.
///
/// Provides centralized loading, lookup, and guaranteed cleanup.
/// All plugins are released when the manager is dropped.
///
/// Supports both native (cdylib) and WebAssembly (Extism) plugins.
/// Plugin names must be unique within a manager instance.
pub struct PluginManager {
    name_index: HashMap<String, usize>,
    plugins: Vec<Option<ManagedPlugin>>,
}

impl PluginManager {
    /// Create an empty PluginManager.
    pub fn new() -> Self {
        Self {
            name_index: HashMap::new(),
            plugins: Vec::new(),
        }
    }

    /// Load a single native plugin file (.dylib/.so/.dll).
    ///
    /// Plugin name resolution:
    /// 1. Try loading the VTable via `plugin_entry` symbol
    /// 2. If successful, use `VTable::name()` for the plugin name
    /// 3. If VTable loading fails (Layer 1 only plugin), derive name from filename
    ///
    /// # Errors
    /// - `PluginError::Load`: library could not be loaded
    /// - `PluginError::DuplicateName`: a plugin with the same name is already loaded
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<&LoadedLibrary, PluginError> {
        let path = path.as_ref();
        let lib = LoadedLibrary::load(path)?;

        // Try to get plugin name from VTable
        let (name, vtable) = match lib.vtable::<PluginVTable>(None) {
            Ok(vt) => {
                let name_ptr = (vt.name)();
                let name = unsafe { std::ffi::CStr::from_ptr(name_ptr) }
                    .to_str()
                    .unwrap_or("unknown")
                    .to_string();
                (name, Some(vt))
            }
            Err(_) => {
                let name = derive_name_from_path(path);
                (name, None)
            }
        };

        // Check for duplicates
        if self.name_index.contains_key(&name) {
            return Err(PluginError::DuplicateName(name));
        }

        // Register
        let native = NativeBackend::new(name.clone(), lib, vtable);
        let idx = self.plugins.len();
        self.plugins.push(Some(ManagedPlugin {
            backend: Box::new(native),
        }));
        self.name_index.insert(name, idx);

        // Return reference to LoadedLibrary inside the NativeBackend
        let managed = self.plugins[idx].as_ref().unwrap();
        let native_ref = managed
            .backend
            .as_any()
            .downcast_ref::<NativeBackend>()
            .unwrap();
        Ok(native_ref.library())
    }

    /// Load a WebAssembly plugin file (.wasm).
    ///
    /// Requires the `wasm` feature flag.
    ///
    /// # Errors
    /// - `PluginError::Load`: Wasm file could not be loaded
    /// - `PluginError::DuplicateName`: a plugin with the same name is already loaded
    #[cfg(feature = "wasm")]
    pub fn load_wasm(&mut self, path: impl AsRef<Path>) -> Result<&str, PluginError> {
        let path = path.as_ref();
        let backend = crate::wasm::WasmBackend::load(path)?;
        let name = backend.name().to_string();

        if self.name_index.contains_key(&name) {
            return Err(PluginError::DuplicateName(name));
        }

        let idx = self.plugins.len();
        self.plugins.push(Some(ManagedPlugin {
            backend: Box::new(backend),
        }));
        self.name_index.insert(name, idx);

        let managed = self.plugins[idx].as_ref().unwrap();
        Ok(managed.backend.name())
    }

    /// Load from an Extism `Manifest` (supports URLs, raw bytes, etc.).
    ///
    /// Requires the `wasm` feature flag.
    #[cfg(feature = "wasm")]
    pub fn load_wasm_manifest(
        &mut self,
        name: impl Into<String>,
        manifest: &extism::Manifest,
        wasi: bool,
    ) -> Result<&str, PluginError> {
        let name_str = name.into();
        let backend = crate::wasm::WasmBackend::load_manifest(name_str.clone(), manifest, wasi)?;

        if self.name_index.contains_key(&name_str) {
            return Err(PluginError::DuplicateName(name_str));
        }

        let idx = self.plugins.len();
        self.plugins.push(Some(ManagedPlugin {
            backend: Box::new(backend),
        }));
        self.name_index.insert(name_str, idx);

        let managed = self.plugins[idx].as_ref().unwrap();
        Ok(managed.backend.name())
    }

    /// Load all shared libraries from a directory (non-recursive).
    ///
    /// Loads both native (.dylib/.so/.dll) and optionally .wasm files.
    /// Individual load failures are logged via `log::warn` and skipped.
    ///
    /// # Returns
    /// Number of successfully loaded plugins.
    pub fn load_from_directory(&mut self, dir: impl AsRef<Path>) -> Result<usize, PluginError> {
        let dir = dir.as_ref();
        let native_ext = crate::platform::lib_extension();
        let mut count = 0;

        let entries = std::fs::read_dir(dir).map_err(PluginError::Io)?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("Failed to read directory entry in {}: {}", dir.display(), e);
                    continue;
                }
            };

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            if ext == native_ext {
                match self.load_file(&path) {
                    Ok(_) => count += 1,
                    Err(e) => {
                        log::warn!("Failed to load native plugin {}: {}", path.display(), e);
                    }
                }
            }

            #[cfg(feature = "wasm")]
            if ext == "wasm" {
                match self.load_wasm(&path) {
                    Ok(_) => count += 1,
                    Err(e) => {
                        log::warn!("Failed to load wasm plugin {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(count)
    }

    /// Load from multiple directories.
    pub fn load_from_directories(
        &mut self,
        dirs: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Result<usize, PluginError> {
        let mut total = 0;
        for dir in dirs {
            total += self.load_from_directory(dir)?;
        }
        Ok(total)
    }

    /// Load from a mix of file paths and directory paths.
    ///
    /// - Directories are scanned via `load_from_directory`
    /// - Files are loaded via `load_file` (native) or `load_wasm` (.wasm)
    /// - Non-existent paths are logged and skipped
    pub fn load_paths(
        &mut self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Result<usize, PluginError> {
        let mut count = 0;
        for path in paths {
            let path = path.as_ref();
            if path.is_dir() {
                count += self.load_from_directory(path)?;
            } else if path.is_file() {
                let loaded = self.try_load_auto(path);
                match loaded {
                    Ok(_) => count += 1,
                    Err(e) => {
                        log::warn!("Failed to load plugin {}: {}", path.display(), e);
                    }
                }
            } else {
                log::warn!("Path does not exist, skipping: {}", path.display());
            }
        }
        Ok(count)
    }

    /// Invoke a method on a loaded plugin by name.
    ///
    /// This is the unified calling interface that works across native and Wasm plugins.
    pub fn invoke(
        &mut self,
        plugin_name: &str,
        method: &str,
        input: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        let idx = self
            .name_index
            .get(plugin_name)
            .copied()
            .ok_or_else(|| PluginError::NotFound(plugin_name.to_string()))?;

        let managed = self.plugins[idx]
            .as_mut()
            .ok_or_else(|| PluginError::NotFound(plugin_name.to_string()))?;

        managed.backend.invoke(method, input)
    }

    /// Look up a loaded native plugin by name.
    ///
    /// Returns `None` if the plugin is not loaded or is a Wasm plugin.
    pub fn get(&self, name: &str) -> Option<&LoadedLibrary> {
        self.name_index
            .get(name)
            .and_then(|&idx| self.plugins[idx].as_ref())
            .and_then(|mp| {
                mp.backend
                    .as_any()
                    .downcast_ref::<NativeBackend>()
                    .map(|nb| nb.library())
            })
    }

    /// List all loaded plugin names.
    pub fn names(&self) -> Vec<&str> {
        self.name_index.keys().map(|s| s.as_str()).collect()
    }

    /// List all loaded native plugins.
    pub fn plugins(&self) -> Vec<&LoadedLibrary> {
        self.plugins
            .iter()
            .filter_map(|opt| {
                opt.as_ref().and_then(|mp| {
                    mp.backend
                        .as_any()
                        .downcast_ref::<NativeBackend>()
                        .map(|nb| nb.library())
                })
            })
            .collect()
    }

    /// Returns the kind of a loaded plugin ("native" or "wasm").
    pub fn plugin_kind(&self, name: &str) -> Option<&'static str> {
        self.name_index
            .get(name)
            .and_then(|&idx| self.plugins[idx].as_ref())
            .map(|mp| mp.backend.kind())
    }

    /// Unload a plugin by name.
    ///
    /// # Errors
    /// - `PluginError::NotFound`: no plugin with that name
    pub fn unload(&mut self, name: &str) -> Result<(), PluginError> {
        let idx = self
            .name_index
            .remove(name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        // Drop the backend (NativeBackend::drop calls vtable.destroy)
        self.plugins[idx].take();

        Ok(())
    }

    /// Unload all plugins in reverse load order.
    pub fn unload_all(&mut self) {
        for i in (0..self.plugins.len()).rev() {
            self.plugins[i].take();
        }
        self.name_index.clear();
        self.plugins.clear();
    }

    /// Auto-detect file type and load accordingly.
    fn try_load_auto(&mut self, path: &Path) -> Result<(), PluginError> {
        #[cfg(feature = "wasm")]
        {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "wasm" {
                self.load_wasm(path)?;
                return Ok(());
            }
        }

        self.load_file(path)?;
        Ok(())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.unload_all();
    }
}

/// Derive plugin name from a library file path.
///
/// Strips the platform prefix ("lib" on Unix) and extension.
/// Example: "libgreeter.dylib" -> "greeter"
fn derive_name_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let prefix = crate::platform::lib_prefix();
    if !prefix.is_empty() && stem.starts_with(prefix) {
        stem[prefix.len()..].to_string()
    } else {
        stem.to_string()
    }
}
