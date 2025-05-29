//! VTable loading and validation (Layer 2).

use crate::api::{INTERFACE_VERSION, PLUGIN_ENTRY_SYMBOL};
use crate::error::PluginError;
use crate::loader::LoadedLibrary;

/// Contract that VTable structs must satisfy.
///
/// # Safety
/// Implementors must ensure:
/// 1. The type is `#[repr(C)]`
/// 2. The first field is `interface_version: u32`
/// 3. All fields are `extern "C" fn` types or C ABI compatible types
///
/// Violating these invariants causes undefined behavior in `vtable()`.
pub unsafe trait VTableValidate {
    /// Returns the interface version stored in this VTable.
    fn interface_version(&self) -> u32;
}

// Implement VTableValidate for the standard PluginVTable
unsafe impl VTableValidate for crate::api::PluginVTable {
    fn interface_version(&self) -> u32 {
        self.interface_version
    }
}

impl LoadedLibrary {
    /// Load a VTable from this library via an entry symbol.
    ///
    /// # Arguments
    /// - `entry_symbol`: Symbol name to look up. Defaults to `"plugin_entry"` if `None`.
    ///
    /// # Process
    /// 1. Look up the entry symbol as `extern "C" fn() -> *const V`
    /// 2. Call it and check the returned pointer is non-null
    /// 3. Compare `VTableValidate::interface_version()` with `INTERFACE_VERSION`
    /// 4. Return `&'static V` on success
    ///
    /// # Lifetime
    /// The returned reference is `'static` but actually points to plugin static memory.
    /// It becomes dangling if the `LoadedLibrary` is dropped.
    /// `PluginManager` prevents this by controlling destroy -> close order.
    ///
    /// # Errors
    /// - `PluginError::SymbolNotFound`: entry symbol not found
    /// - `PluginError::NullVTable`: entry function returned null
    /// - `PluginError::VersionMismatch`: interface version mismatch
    pub fn vtable<V: VTableValidate>(
        &self,
        entry_symbol: Option<&str>,
    ) -> Result<&'static V, PluginError> {
        let symbol_name = entry_symbol.unwrap_or(PLUGIN_ENTRY_SYMBOL);

        // 1. Look up entry function
        let entry_fn = self.bind::<extern "C" fn() -> *const V>(symbol_name)?;

        // 2. Call entry function
        let vtable_ptr = entry_fn();

        // 3. Null check
        if vtable_ptr.is_null() {
            return Err(PluginError::NullVTable {
                path: self.path().display().to_string(),
            });
        }

        // 4. Convert to &'static V
        let vtable = unsafe { &*vtable_ptr };

        // 5. Version check
        if vtable.interface_version() != INTERFACE_VERSION {
            return Err(PluginError::VersionMismatch {
                host: INTERFACE_VERSION,
                plugin: vtable.interface_version(),
                path: self.path().display().to_string(),
            });
        }

        Ok(vtable)
    }
}
