//! Integration tests for dynplug using the dynplug-example plugin.
//!
//! Prerequisites: `cargo build -p dynplug-example` must be run before these tests.

use std::path::PathBuf;

/// Helper to find the cdylib path.
fn plugin_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join(dynplug::lib_filename("dynplug-example"))
}

// =============================================
// Layer 1: LoadedLibrary + bind
// =============================================

#[test]
fn test_load_and_bind_entry() {
    let path = plugin_path();
    let lib = dynplug::LoadedLibrary::load(&path).expect("failed to load plugin");
    let entry = lib
        .bind::<dynplug::PluginEntryFn>("plugin_entry")
        .expect("failed to bind plugin_entry");
    let vtable_ptr = entry();
    assert!(!vtable_ptr.is_null(), "plugin_entry returned null");
}

// =============================================
// Layer 2: VTable
// =============================================

#[test]
fn test_vtable_load_and_version_check() {
    let path = plugin_path();
    let lib = dynplug::LoadedLibrary::load(&path).unwrap();
    let vt = lib
        .vtable::<dynplug::PluginVTable>(None)
        .expect("failed to load vtable");

    // Check name
    let name =
        unsafe { std::ffi::CStr::from_ptr((vt.name)()) }
            .to_str()
            .unwrap();
    assert_eq!(name, "greeter");

    // Check version
    assert_eq!((vt.version)(), 1);

    // Check interface version
    assert_eq!(vt.interface_version, dynplug::INTERFACE_VERSION);
}

#[test]
fn test_invoke_greet() {
    let path = plugin_path();
    let lib = dynplug::LoadedLibrary::load(&path).unwrap();
    let vt = lib.vtable::<dynplug::PluginVTable>(None).unwrap();

    let method = b"greet";
    let input = b"World";
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

    assert_eq!(rc, 0);
    assert!(!out_ptr.is_null());
    let output = unsafe { std::slice::from_raw_parts(out_ptr, out_len) };
    assert_eq!(std::str::from_utf8(output).unwrap(), "Hello, World!");
    (vt.free_buffer)(out_ptr, out_len);
}

#[test]
fn test_invoke_add() {
    let path = plugin_path();
    let lib = dynplug::LoadedLibrary::load(&path).unwrap();
    let vt = lib.vtable::<dynplug::PluginVTable>(None).unwrap();

    let method = b"add";
    let mut input = Vec::new();
    input.extend_from_slice(&21_i32.to_le_bytes());
    input.extend_from_slice(&21_i32.to_le_bytes());
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

    assert_eq!(rc, 0);
    let result = i32::from_le_bytes(
        unsafe { std::slice::from_raw_parts(out_ptr, out_len) }
            .try_into()
            .unwrap(),
    );
    assert_eq!(result, 42);
    (vt.free_buffer)(out_ptr, out_len);
}

#[test]
fn test_invoke_noop_empty_output() {
    let path = plugin_path();
    let lib = dynplug::LoadedLibrary::load(&path).unwrap();
    let vt = lib.vtable::<dynplug::PluginVTable>(None).unwrap();

    let method = b"noop";
    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let rc = (vt.invoke)(
        method.as_ptr(),
        method.len(),
        std::ptr::null(),
        0,
        &mut out_ptr,
        &mut out_len,
    );

    assert_eq!(rc, 0);
    assert!(out_ptr.is_null());
    assert_eq!(out_len, 0);
    // No free_buffer needed for empty output
}

// =============================================
// Error cases
// =============================================

#[test]
fn test_invoke_unknown_method_returns_error() {
    let path = plugin_path();
    let lib = dynplug::LoadedLibrary::load(&path).unwrap();
    let vt = lib.vtable::<dynplug::PluginVTable>(None).unwrap();

    let method = b"unknown";
    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let rc = (vt.invoke)(
        method.as_ptr(),
        method.len(),
        std::ptr::null(),
        0,
        &mut out_ptr,
        &mut out_len,
    );

    assert_eq!(rc, -1);
    assert!(!out_ptr.is_null());
    let err_msg = unsafe { std::slice::from_raw_parts(out_ptr, out_len) };
    let err_str = std::str::from_utf8(err_msg).unwrap();
    assert!(err_str.contains("unknown method"), "got: {err_str}");
    (vt.free_buffer)(out_ptr, out_len);
}

#[test]
fn test_invoke_panic_returns_minus2() {
    let path = plugin_path();
    let lib = dynplug::LoadedLibrary::load(&path).unwrap();
    let vt = lib.vtable::<dynplug::PluginVTable>(None).unwrap();

    let method = b"panic_test";
    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let rc = (vt.invoke)(
        method.as_ptr(),
        method.len(),
        std::ptr::null(),
        0,
        &mut out_ptr,
        &mut out_len,
    );

    assert_eq!(rc, -2);
    // out_ptr/out_len are undefined after panic — do NOT call free_buffer
}

#[test]
fn test_load_nonexistent_file() {
    let result = dynplug::LoadedLibrary::load("/nonexistent/path.dylib");
    let err = result.err().expect("expected an error");
    assert!(matches!(err, dynplug::PluginError::Load { .. }));
}

#[test]
fn test_bind_nonexistent_symbol() {
    let path = plugin_path();
    let lib = dynplug::LoadedLibrary::load(&path).unwrap();
    let result = lib.bind::<extern "C" fn()>("no_such_symbol");
    let err = result.err().expect("expected an error");
    assert!(matches!(err, dynplug::PluginError::SymbolNotFound { .. }));
}

#[test]
fn test_vtable_with_wrong_entry_symbol() {
    let path = plugin_path();
    let lib = dynplug::LoadedLibrary::load(&path).unwrap();
    let result = lib.vtable::<dynplug::PluginVTable>(Some("nonexistent"));
    let err = result.err().expect("expected an error");
    assert!(matches!(err, dynplug::PluginError::SymbolNotFound { .. }));
}
