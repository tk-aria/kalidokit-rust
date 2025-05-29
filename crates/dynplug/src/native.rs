//! Native cdylib plugin backend.

use std::path::Path;

use crate::api::PluginVTable;
use crate::backend::PluginBackend;
use crate::error::PluginError;
use crate::loader::LoadedLibrary;

/// A native (cdylib) plugin loaded via the standard dynplug VTable protocol.
pub struct NativeBackend {
    name: String,
    library: LoadedLibrary,
    vtable: Option<&'static PluginVTable>,
}

impl NativeBackend {
    /// Create a NativeBackend from an already-loaded library.
    pub(crate) fn new(
        name: String,
        library: LoadedLibrary,
        vtable: Option<&'static PluginVTable>,
    ) -> Self {
        Self {
            name,
            library,
            vtable,
        }
    }

    /// Returns a reference to the underlying `LoadedLibrary`.
    pub fn library(&self) -> &LoadedLibrary {
        &self.library
    }

    /// Returns the VTable if this plugin exports one.
    pub fn vtable(&self) -> Option<&'static PluginVTable> {
        self.vtable
    }
}

impl PluginBackend for NativeBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn invoke(&mut self, method: &str, input: &[u8]) -> Result<Vec<u8>, PluginError> {
        let vt = self.vtable.ok_or_else(|| PluginError::Invoke {
            message: "plugin has no VTable".to_string(),
        })?;

        let mut out_ptr: *mut u8 = std::ptr::null_mut();
        let mut out_len: usize = 0;

        let rc = (vt.invoke)(
            method.as_ptr(),
            method.len(),
            input.as_ptr(),
            input.len(),
            &mut out_ptr,
            &mut out_len,
        );

        match rc {
            0 => {
                if out_ptr.is_null() || out_len == 0 {
                    Ok(Vec::new())
                } else {
                    let data = unsafe { std::slice::from_raw_parts(out_ptr, out_len) }.to_vec();
                    (vt.free_buffer)(out_ptr, out_len);
                    Ok(data)
                }
            }
            -1 => {
                let msg = if !out_ptr.is_null() && out_len > 0 {
                    let s = unsafe { std::slice::from_raw_parts(out_ptr, out_len) };
                    let msg = String::from_utf8_lossy(s).to_string();
                    (vt.free_buffer)(out_ptr, out_len);
                    msg
                } else {
                    "unknown error".to_string()
                };
                Err(PluginError::Invoke { message: msg })
            }
            -2 => Err(PluginError::Panic),
            _ => Err(PluginError::Invoke {
                message: format!("unexpected return code: {rc}"),
            }),
        }
    }

    fn path(&self) -> Option<&Path> {
        Some(self.library.path())
    }

    fn kind(&self) -> &'static str {
        "native"
    }
}

impl Drop for NativeBackend {
    fn drop(&mut self) {
        if let Some(vt) = self.vtable {
            (vt.destroy)();
        }
    }
}
