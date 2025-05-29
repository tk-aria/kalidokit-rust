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
    let name = unsafe { std::ffi::CStr::from_ptr((vt.name)()) }
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

/// Panic test is run in a subprocess to avoid corrupting the test process.
/// After catch_unwind in a cdylib, the library's TLS state may be corrupted,
/// causing SIGSEGV on dlclose or subsequent loads in the same process.
#[test]
fn test_invoke_panic_returns_minus2() {
    let exe = std::env::current_exe().unwrap();
    let output = std::process::Command::new(exe)
        .arg("--ignored")
        .arg("test_invoke_panic_subprocess")
        .arg("--test-threads=1")
        .arg("--exact")
        .env("RUST_TEST_THREADS", "1")
        .output()
        .expect("failed to spawn subprocess");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("ok"),
        "subprocess panic test failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
#[ignore]
fn test_invoke_panic_subprocess() {
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
    // Leak library to avoid SIGSEGV from dlclose after caught panic
    std::mem::forget(lib);
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

// =============================================
// PluginManager
// =============================================

#[test]
fn test_manager_load_file_and_get() {
    let path = plugin_path();
    let mut manager = dynplug::PluginManager::new();
    let _lib = manager.load_file(&path).expect("load_file failed");

    let found = manager.get("greeter");
    assert!(found.is_some(), "get('greeter') should return Some");
}

#[test]
fn test_manager_names() {
    let path = plugin_path();
    let mut manager = dynplug::PluginManager::new();
    manager.load_file(&path).unwrap();

    let names = manager.names();
    assert!(names.contains(&"greeter"), "names should contain 'greeter'");
}

#[test]
fn test_manager_plugins() {
    let path = plugin_path();
    let mut manager = dynplug::PluginManager::new();
    manager.load_file(&path).unwrap();

    let plugins = manager.plugins();
    assert_eq!(plugins.len(), 1, "should have 1 plugin loaded");
}

#[test]
fn test_manager_unload() {
    let path = plugin_path();
    let mut manager = dynplug::PluginManager::new();
    manager.load_file(&path).unwrap();

    manager.unload("greeter").expect("unload failed");
    assert!(
        manager.get("greeter").is_none(),
        "should be None after unload"
    );
}

#[test]
fn test_manager_load_from_directory() {
    let path = plugin_path();
    // Use a temp directory with a symlink to avoid loading incompatible L3 plugins
    let tmp = std::env::temp_dir().join("dynplug_test_dir");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let link = tmp.join(path.file_name().unwrap());
    // Copy instead of symlink for portability
    std::fs::copy(&path, &link).unwrap();

    let mut manager = dynplug::PluginManager::new();
    let count = manager
        .load_from_directory(&tmp)
        .expect("load_from_directory failed");
    assert!(count >= 1, "should load at least 1 plugin from directory");

    drop(manager);
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_manager_load_paths_mixed() {
    let path = plugin_path();
    let mut manager = dynplug::PluginManager::new();

    // Load by file path
    let count = manager
        .load_paths([path.as_path()])
        .expect("load_paths failed");
    assert_eq!(count, 1);
}

#[test]
fn test_manager_drop_releases_all() {
    let path = plugin_path();
    {
        let mut manager = dynplug::PluginManager::new();
        manager.load_file(&path).unwrap();
        // manager dropped here
    }
    // Verify we can reload (library was closed)
    let lib = dynplug::LoadedLibrary::load(&path);
    assert!(lib.is_ok(), "should be able to reload after manager drop");
}

// --- Error cases ---

#[test]
fn test_manager_duplicate_name() {
    let path = plugin_path();
    let mut manager = dynplug::PluginManager::new();
    manager.load_file(&path).unwrap();

    let result = manager.load_file(&path);
    assert!(result.is_err());
    let err = result.err().expect("should be DuplicateName error");
    assert!(
        matches!(err, dynplug::PluginError::DuplicateName(_)),
        "expected DuplicateName, got: {err}"
    );
}

#[test]
fn test_manager_unload_nonexistent() {
    let mut manager = dynplug::PluginManager::new();
    let result = manager.unload("no_such");
    assert!(result.is_err());
    let err = result.err().expect("should be NotFound error");
    assert!(
        matches!(err, dynplug::PluginError::NotFound(_)),
        "expected NotFound, got: {err}"
    );
}

#[test]
fn test_manager_load_from_nonexistent_directory() {
    let mut manager = dynplug::PluginManager::new();
    let result = manager.load_from_directory("/nonexistent/directory");
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), dynplug::PluginError::Io(_)));
}

#[test]
fn test_manager_load_paths_nonexistent_skipped() {
    let path = plugin_path();
    let mut manager = dynplug::PluginManager::new();
    let nonexistent = std::path::PathBuf::from("/nonexistent/plugin.dylib");

    // Mix real path with nonexistent — nonexistent should be skipped
    let count = manager
        .load_paths([path.as_path(), nonexistent.as_path()])
        .expect("load_paths should not fail for skipped paths");
    assert_eq!(count, 1, "only the real plugin should be loaded");
}
