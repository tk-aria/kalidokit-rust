//! Layer 3: Safe wrapper macro for automatic VTable + wrapper generation.
//!
//! For v0.1, supports only primitive types (i32, u32, i64, u64, f32, f64, bool, usize, isize)
//! in function signatures. For &str/String support, use Layer 2 directly.

/// Generate a `#[repr(C)]` VTable struct, `VTableValidate` impl, and host-side wrapper.
///
/// # Example
/// ```ignore
/// dynplug::define_plugin! {
///     pub struct Greeter {
///         fn add(a: i32, b: i32) -> i32;
///     }
/// }
///
/// // This generates:
/// // - GreeterVTable: #[repr(C)] struct with function pointers
/// // - Greeter: wrapper with load(path) and safe method calls
/// // - VTableValidate impl for GreeterVTable
///
/// let g = Greeter::load("path/to/plugin")?;
/// assert_eq!(g.add(21, 21), 42);
/// ```
///
/// # Plugin side (manual)
/// ```ignore
/// // Mirror the generated VTable layout in the plugin crate:
/// #[repr(C)]
/// pub struct GreeterVTable {
///     pub interface_version: u32,
///     pub add: extern "C" fn(i32, i32) -> i32,
///     pub destroy: extern "C" fn(),
/// }
///
/// extern "C" fn my_add(a: i32, b: i32) -> i32 { a + b }
/// extern "C" fn my_destroy() {}
///
/// static VTABLE: GreeterVTable = GreeterVTable {
///     interface_version: 1,
///     add: my_add,
///     destroy: my_destroy,
/// };
///
/// #[no_mangle]
/// pub extern "C" fn plugin_entry() -> *const GreeterVTable { &VTABLE }
/// ```
#[macro_export]
macro_rules! define_plugin {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $(fn $fn_name:ident( $($arg_name:ident : $arg_ty:ty),* $(,)? ) $(-> $ret_ty:ty)?;)*
        }
    ) => {
        $crate::paste::paste! {
            /// Auto-generated VTable for the plugin interface.
            #[repr(C)]
            $(#[$meta])*
            pub struct [<$name VTable>] {
                /// Interface version. Must match `dynplug::INTERFACE_VERSION`.
                pub interface_version: u32,
                $(
                    pub $fn_name: extern "C" fn($($arg_ty),*) $(-> $ret_ty)?,
                )*
                /// Plugin cleanup function. Called before library close.
                pub destroy: extern "C" fn(),
            }

            // SAFETY: The generated struct is #[repr(C)] with interface_version as the first field,
            // and all fields are extern "C" fn types.
            unsafe impl $crate::VTableValidate for [<$name VTable>] {
                fn interface_version(&self) -> u32 {
                    self.interface_version
                }
            }

            /// Auto-generated host-side wrapper.
            $(#[$meta])*
            pub struct $name {
                _lib: $crate::LoadedLibrary,
                vtable: &'static [<$name VTable>],
            }

            impl $name {
                /// Load a plugin implementing this interface from the given path.
                pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, $crate::PluginError> {
                    let lib = $crate::LoadedLibrary::load(path)?;
                    let vtable = lib.vtable::<[<$name VTable>]>(None)?;
                    Ok(Self { _lib: lib, vtable })
                }

                $(
                    pub fn $fn_name(&self, $($arg_name: $arg_ty),*) $(-> $ret_ty)? {
                        (self.vtable.$fn_name)($($arg_name),*)
                    }
                )*
            }

            impl Drop for $name {
                fn drop(&mut self) {
                    (self.vtable.destroy)();
                }
            }
        }
    };
}
