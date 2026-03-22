//! Plugin export macro for generating C ABI bridge code.

/// Generates C ABI entry point and VTable for a dynplug plugin.
///
/// # Usage
/// ```ignore
/// dynplug::export_plugin! {
///     name: "greeter",
///     version: 1,
///     invoke: handle_invoke,
/// }
///
/// fn handle_invoke(method: &str, input: &[u8]) -> Result<Vec<u8>, String> {
///     // ...
/// }
/// ```
#[macro_export]
macro_rules! export_plugin {
    (
        name: $name:literal,
        version: $ver:expr,
        invoke: $invoke:path $(,)?
    ) => {
        mod __dynplug_generated {
            use super::*;

            pub extern "C" fn name() -> *const std::os::raw::c_char {
                concat!($name, "\0").as_ptr() as *const std::os::raw::c_char
            }

            pub extern "C" fn version() -> u32 {
                $ver
            }

            pub extern "C" fn invoke(
                method_ptr: *const u8,
                method_len: usize,
                input_ptr: *const u8,
                input_len: usize,
                out_ptr: *mut *mut u8,
                out_len: *mut usize,
            ) -> i32 {
                let result = std::panic::catch_unwind(|| {
                    let method = unsafe {
                        std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                            method_ptr, method_len,
                        ))
                    };
                    let input = if input_len == 0 {
                        &[]
                    } else {
                        unsafe { std::slice::from_raw_parts(input_ptr, input_len) }
                    };
                    $invoke(method, input)
                });

                match result {
                    Ok(Ok(buf)) => {
                        if buf.is_empty() {
                            unsafe {
                                *out_ptr = std::ptr::null_mut();
                                *out_len = 0;
                            }
                        } else {
                            let boxed = buf.into_boxed_slice();
                            let len = boxed.len();
                            let ptr = Box::into_raw(boxed) as *mut u8;
                            unsafe {
                                *out_ptr = ptr;
                                *out_len = len;
                            }
                        }
                        0
                    }
                    Ok(Err(msg)) => {
                        let bytes = msg.into_bytes().into_boxed_slice();
                        let len = bytes.len();
                        let ptr = Box::into_raw(bytes) as *mut u8;
                        unsafe {
                            *out_ptr = ptr;
                            *out_len = len;
                        }
                        -1
                    }
                    Err(_) => {
                        // Panic caught — do NOT touch out_ptr/out_len
                        -2
                    }
                }
            }

            pub extern "C" fn free_buffer(ptr: *mut u8, len: usize) {
                if !ptr.is_null() && len > 0 {
                    unsafe {
                        drop(Box::from_raw(std::slice::from_raw_parts_mut(ptr, len)));
                    }
                }
            }

            pub extern "C" fn destroy() {
                // Default: no-op
            }

            #[used]
            static VTABLE: $crate::PluginVTable = $crate::PluginVTable {
                interface_version: $crate::INTERFACE_VERSION,
                name,
                version,
                invoke,
                free_buffer,
                destroy,
            };

            #[no_mangle]
            pub extern "C" fn plugin_entry() -> *const $crate::PluginVTable {
                &VTABLE
            }
        }
    };
}
