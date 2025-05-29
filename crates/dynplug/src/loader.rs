//! Library loading and symbol binding (Layer 0 + Layer 1).

use std::path::{Path, PathBuf};

use crate::error::PluginError;

/// A loaded shared library.
///
/// Manages the lifetime of a dynamically loaded library.
/// When dropped, the underlying library is closed.
///
/// If using VTable references obtained from this library,
/// ensure they are discarded before dropping the `LoadedLibrary`.
/// (`PluginManager` handles this automatically.)
pub struct LoadedLibrary {
    lib: libloading::Library,
    path: PathBuf,
}

impl LoadedLibrary {
    /// Load a shared library from the given path.
    ///
    /// Only calls the OS loader (dlopen / LoadLibrary) at this point.
    /// No symbol lookup or version check is performed.
    ///
    /// # Errors
    /// - `PluginError::Load`: file does not exist, permission denied, invalid binary, etc.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PluginError> {
        let path = path.as_ref();
        let lib = unsafe {
            libloading::Library::new(path).map_err(|e| PluginError::Load {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?
        };
        Ok(Self {
            lib,
            path: path.to_path_buf(),
        })
    }

    /// Returns the path this library was loaded from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bind a symbol from this library to a typed function handle.
    ///
    /// The type parameter `F` should be an `extern "C" fn(...)` type.
    /// The caller is responsible for specifying the correct type;
    /// a type mismatch leads to undefined behavior at call time.
    ///
    /// For type-safe access, prefer Layer 2 (`vtable()`) or Layer 3 (`define_plugin!`).
    ///
    /// # Errors
    /// - `PluginError::SymbolNotFound`: no symbol with the given name exists
    pub fn bind<F>(&self, name: &str) -> Result<BoundFn<'_, F>, PluginError> {
        let c_name = std::ffi::CString::new(name).map_err(|_| PluginError::SymbolNotFound {
            symbol: name.to_string(),
            path: self.path.display().to_string(),
        })?;
        unsafe {
            let sym = self.lib.get::<F>(c_name.as_bytes_with_nul()).map_err(|_| {
                PluginError::SymbolNotFound {
                    symbol: name.to_string(),
                    path: self.path.display().to_string(),
                }
            })?;
            Ok(BoundFn { sym })
        }
    }
}

/// A bound function handle tied to a loaded library's lifetime.
///
/// Implements `Deref<Target = F>` so it can be called like a regular function pointer.
///
/// # Example
/// ```ignore
/// let add = lib.bind::<extern "C" fn(i32, i32) -> i32>("add")?;
/// let result = add(21, 21); // Deref coercion makes this work
/// ```
pub struct BoundFn<'lib, F> {
    sym: libloading::Symbol<'lib, F>,
}

impl<F> std::ops::Deref for BoundFn<'_, F> {
    type Target = F;
    fn deref(&self) -> &F {
        &self.sym
    }
}
