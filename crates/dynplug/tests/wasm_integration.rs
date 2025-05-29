//! Integration tests for dynplug Wasm backend.
//!
//! Prerequisites:
//! - `cargo build -p dynplug-example-wasm --target wasm32-unknown-unknown`
//! - Run with: `cargo test -p dynplug --features wasm`
#![cfg(feature = "wasm")]

use dynplug::PluginBackend;
use std::path::PathBuf;

/// Helper to find the wasm plugin path.
fn wasm_plugin_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("debug")
        .join("dynplug_example_wasm.wasm")
}

/// Helper to find the native cdylib path.
fn native_plugin_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join(dynplug::lib_filename("dynplug-example"))
}

// =============================================
// WasmBackend direct tests
// =============================================

#[test]
fn test_wasm_load() {
    let path = wasm_plugin_path();
    let backend = dynplug::WasmBackend::load(&path);
    assert!(
        backend.is_ok(),
        "failed to load wasm plugin: {:?}",
        backend.err()
    );
}

#[test]
fn test_wasm_name() {
    let path = wasm_plugin_path();
    let backend = dynplug::WasmBackend::load(&path).unwrap();
    assert_eq!(backend.name(), "dynplug_example_wasm");
}

#[test]
fn test_wasm_invoke_greet() {
    let path = wasm_plugin_path();
    let mut backend = dynplug::WasmBackend::load(&path).unwrap();
    let result = backend.invoke("greet", b"World").unwrap();
    assert_eq!(std::str::from_utf8(&result).unwrap(), "Hello, World!");
}

#[test]
fn test_wasm_invoke_add() {
    let path = wasm_plugin_path();
    let mut backend = dynplug::WasmBackend::load(&path).unwrap();

    let mut input = Vec::new();
    input.extend_from_slice(&21_i32.to_le_bytes());
    input.extend_from_slice(&21_i32.to_le_bytes());

    let result = backend.invoke("add", &input).unwrap();
    let sum = i32::from_le_bytes(result.try_into().unwrap());
    assert_eq!(sum, 42);
}

#[test]
fn test_wasm_invoke_noop() {
    let path = wasm_plugin_path();
    let mut backend = dynplug::WasmBackend::load(&path).unwrap();
    let result = backend.invoke("noop", &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_wasm_invoke_unknown_method() {
    let path = wasm_plugin_path();
    let mut backend = dynplug::WasmBackend::load(&path).unwrap();
    let result = backend.invoke("nonexistent", &[]);
    assert!(result.is_err());
}

#[test]
fn test_wasm_load_nonexistent() {
    let result = dynplug::WasmBackend::load("/nonexistent/plugin.wasm");
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(matches!(err, dynplug::PluginError::Load { .. }));
}

#[test]
fn test_wasm_kind() {
    let path = wasm_plugin_path();
    let backend = dynplug::WasmBackend::load(&path).unwrap();
    assert_eq!(backend.kind(), "wasm");
}

// =============================================
// PluginManager with Wasm
// =============================================

#[test]
fn test_manager_load_wasm() {
    let path = wasm_plugin_path();
    let mut manager = dynplug::PluginManager::new();
    let name = manager.load_wasm(&path).expect("load_wasm failed");
    assert_eq!(name, "dynplug_example_wasm");
    assert!(manager.names().contains(&"dynplug_example_wasm"));
}

#[test]
fn test_manager_invoke_wasm() {
    let path = wasm_plugin_path();
    let mut manager = dynplug::PluginManager::new();
    manager.load_wasm(&path).unwrap();

    let result = manager
        .invoke("dynplug_example_wasm", "greet", b"Rust")
        .unwrap();
    assert_eq!(std::str::from_utf8(&result).unwrap(), "Hello, Rust!");
}

#[test]
fn test_manager_invoke_native() {
    let path = native_plugin_path();
    let mut manager = dynplug::PluginManager::new();
    manager.load_file(&path).unwrap();

    let result = manager.invoke("greeter", "greet", b"World").unwrap();
    assert_eq!(std::str::from_utf8(&result).unwrap(), "Hello, World!");
}

#[test]
fn test_manager_mixed_plugins() {
    let wasm_path = wasm_plugin_path();
    let native_path = native_plugin_path();

    let mut manager = dynplug::PluginManager::new();
    manager.load_file(&native_path).unwrap();
    manager.load_wasm(&wasm_path).unwrap();

    let names = manager.names();
    assert!(names.contains(&"greeter"));
    assert!(names.contains(&"dynplug_example_wasm"));

    // Invoke on both
    let r1 = manager.invoke("greeter", "greet", b"Native").unwrap();
    assert_eq!(std::str::from_utf8(&r1).unwrap(), "Hello, Native!");

    let r2 = manager
        .invoke("dynplug_example_wasm", "greet", b"Wasm")
        .unwrap();
    assert_eq!(std::str::from_utf8(&r2).unwrap(), "Hello, Wasm!");
}

#[test]
fn test_manager_plugin_kind() {
    let wasm_path = wasm_plugin_path();
    let native_path = native_plugin_path();

    let mut manager = dynplug::PluginManager::new();
    manager.load_file(&native_path).unwrap();
    manager.load_wasm(&wasm_path).unwrap();

    assert_eq!(manager.plugin_kind("greeter"), Some("native"));
    assert_eq!(manager.plugin_kind("dynplug_example_wasm"), Some("wasm"));
}

#[test]
fn test_manager_unload_wasm() {
    let path = wasm_plugin_path();
    let mut manager = dynplug::PluginManager::new();
    manager.load_wasm(&path).unwrap();

    manager
        .unload("dynplug_example_wasm")
        .expect("unload failed");
    assert!(manager.names().is_empty());
}

#[test]
fn test_manager_wasm_duplicate_name() {
    let path = wasm_plugin_path();
    let mut manager = dynplug::PluginManager::new();
    manager.load_wasm(&path).unwrap();

    let result = manager.load_wasm(&path);
    assert!(result.is_err());
    assert!(matches!(
        result.err().unwrap(),
        dynplug::PluginError::DuplicateName(_)
    ));
}

#[test]
fn test_manager_directory_mixed() {
    let wasm_path = wasm_plugin_path();
    let native_path = native_plugin_path();

    // Create a temp dir with both native and wasm plugins
    let tmp = std::env::temp_dir().join("dynplug_test_mixed");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::copy(&native_path, tmp.join(native_path.file_name().unwrap())).unwrap();
    std::fs::copy(&wasm_path, tmp.join(wasm_path.file_name().unwrap())).unwrap();

    let mut manager = dynplug::PluginManager::new();
    let count = manager
        .load_from_directory(&tmp)
        .expect("load_from_directory failed");
    assert!(count >= 2, "expected at least 2 plugins, got {count}");

    drop(manager);
    let _ = std::fs::remove_dir_all(&tmp);
}
