//! Centralized plugin lifecycle management.

use std::collections::HashMap;
use std::path::Path;

use crate::api::PluginVTable;
use crate::error::PluginError;
use crate::loader::LoadedLibrary;

/// Internal record for a managed plugin.
struct ManagedPlugin {
    library: LoadedLibrary,
    vtable: Option<&'static PluginVTable>,
}

/// Manages the lifecycle of multiple plugins.
///
/// Provides centralized loading, lookup, and guaranteed cleanup.
/// All plugins are released (destroy + close) when the manager is dropped.
///
/// Plugin names must be unique within a manager instance.
pub struct PluginManager {
    name_index: HashMap<String, usize>,
    libraries: Vec<Option<ManagedPlugin>>,
}

impl PluginManager {
    /// Create an empty PluginManager.
    pub fn new() -> Self {
        Self {
            name_index: HashMap::new(),
            libraries: Vec::new(),
        }
    }

    /// Load a single plugin file.
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
                // Layer 1 only plugin — derive name from filename
                let name = derive_name_from_path(path);
                (name, None)
            }
        };

        // Check for duplicates
        if self.name_index.contains_key(&name) {
            return Err(PluginError::DuplicateName(name));
        }

        // Register
        let idx = self.libraries.len();
        self.libraries.push(Some(ManagedPlugin {
            library: lib,
            vtable,
        }));
        self.name_index.insert(name, idx);

        Ok(&self.libraries[idx].as_ref().unwrap().library)
    }

    /// Load all shared libraries from a directory (non-recursive).
    ///
    /// Only files matching the platform's library extension are loaded.
    /// Individual load failures are logged via `log::warn` and skipped.
    ///
    /// # Returns
    /// Number of successfully loaded plugins.
    pub fn load_from_directory(&mut self, dir: impl AsRef<Path>) -> Result<usize, PluginError> {
        let dir = dir.as_ref();
        let ext = crate::platform::lib_extension();
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
            if path.is_file() {
                if let Some(file_ext) = path.extension() {
                    if file_ext == ext {
                        match self.load_file(&path) {
                            Ok(_) => count += 1,
                            Err(e) => {
                                log::warn!("Failed to load plugin {}: {}", path.display(), e);
                            }
                        }
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
    /// - Files are loaded via `load_file`
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
                match self.load_file(path) {
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

    /// Look up a loaded plugin by name.
    pub fn get(&self, name: &str) -> Option<&LoadedLibrary> {
        self.name_index
            .get(name)
            .and_then(|&idx| self.libraries[idx].as_ref())
            .map(|mp| &mp.library)
    }

    /// List all loaded plugin names.
    pub fn names(&self) -> Vec<&str> {
        self.name_index.keys().map(|s| s.as_str()).collect()
    }

    /// List all loaded plugins.
    pub fn plugins(&self) -> Vec<&LoadedLibrary> {
        self.libraries
            .iter()
            .filter_map(|opt| opt.as_ref().map(|mp| &mp.library))
            .collect()
    }

    /// Unload a plugin by name.
    ///
    /// Calls `destroy()` if VTable is available, then drops the library.
    ///
    /// # Errors
    /// - `PluginError::NotFound`: no plugin with that name
    pub fn unload(&mut self, name: &str) -> Result<(), PluginError> {
        let idx = self
            .name_index
            .remove(name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        if let Some(managed) = self.libraries[idx].take() {
            if let Some(vt) = managed.vtable {
                (vt.destroy)();
            }
            // managed.library is dropped here -> Library::close()
        }

        Ok(())
    }

    /// Unload all plugins in reverse load order.
    pub fn unload_all(&mut self) {
        // Reverse order for proper cleanup
        for i in (0..self.libraries.len()).rev() {
            if let Some(managed) = self.libraries[i].take() {
                if let Some(vt) = managed.vtable {
                    (vt.destroy)();
                }
                // managed.library dropped -> Library::close()
            }
        }
        self.name_index.clear();
        self.libraries.clear();
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
