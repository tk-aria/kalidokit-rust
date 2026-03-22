//! Layer 3 example plugin - manual VTable matching define_plugin! generated shape.
//!
//! This VTable layout must match what `define_plugin!` generates for:
//! ```ignore
//! dynplug::define_plugin! {
//!     pub struct Calculator {
//!         fn add(a: i32, b: i32) -> i32;
//!         fn multiply(a: i32, b: i32) -> i32;
//!         fn negate(x: i32) -> i32;
//!     }
//! }
//! ```

#[repr(C)]
pub struct CalculatorVTable {
    pub interface_version: u32,
    pub add: extern "C" fn(i32, i32) -> i32,
    pub multiply: extern "C" fn(i32, i32) -> i32,
    pub negate: extern "C" fn(i32) -> i32,
    pub destroy: extern "C" fn(),
}

extern "C" fn my_add(a: i32, b: i32) -> i32 {
    a + b
}

extern "C" fn my_multiply(a: i32, b: i32) -> i32 {
    a * b
}

extern "C" fn my_negate(x: i32) -> i32 {
    -x
}

extern "C" fn my_destroy() {}

static VTABLE: CalculatorVTable = CalculatorVTable {
    interface_version: dynplug::INTERFACE_VERSION,
    add: my_add,
    multiply: my_multiply,
    negate: my_negate,
    destroy: my_destroy,
};

#[no_mangle]
pub extern "C" fn plugin_entry() -> *const CalculatorVTable {
    &VTABLE
}
