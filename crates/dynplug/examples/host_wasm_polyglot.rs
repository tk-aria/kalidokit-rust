//! Polyglot host example: Rust (native), Rust (wasm), Python (wasm), JavaScript (wasm).
//!
//! Prerequisites:
//!   cargo build -p dynplug-example
//!   cargo build -p dynplug-example-wasm --target wasm32-unknown-unknown
//!   cd examples/plugins/python  && extism-py plugin.py -o plugin.wasm
//!   cd examples/plugins/javascript && extism-js plugin.js -i plugin.d.ts -o plugin.wasm
//!
//! Run:
//!   cargo run -p dynplug --features wasm --example host_wasm_polyglot

#[cfg(not(feature = "wasm"))]
fn main() {
    eprintln!("This example requires the `wasm` feature.");
    eprintln!("Run: cargo run -p dynplug --features wasm --example host_wasm_polyglot");
    std::process::exit(1);
}

#[cfg(feature = "wasm")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== dynplug Polyglot Host Example ===\n");

    let mut manager = dynplug::PluginManager::new();

    // ========================================
    // 1. Native Rust plugin (cdylib)
    // ========================================
    let native_path = find_native_path();
    println!("--- Native Rust plugin ---");
    println!("Path: {}", native_path.display());
    manager.load_file(&native_path)?;
    println!("[LOADED] greeter (native)");

    // ========================================
    // 2. Rust Wasm plugin (extism-pdk)
    // ========================================
    let rust_wasm_path = find_rust_wasm_path();
    println!("\n--- Rust Wasm plugin ---");
    println!("Path: {}", rust_wasm_path.display());
    manager.load_wasm(&rust_wasm_path)?;
    println!("[LOADED] dynplug_example_wasm (wasm/rust)");

    // ========================================
    // 3. Python Wasm plugin (extism-py)
    // ========================================
    let python_path = find_plugin_dir().join("python").join("plugin.wasm");
    println!("\n--- Python Wasm plugin ---");
    println!("Path: {}", python_path.display());
    if python_path.exists() {
        manager.load_wasm(&python_path)?;
        println!("[LOADED] plugin (wasm/python)");
    } else {
        println!("[SKIP] plugin.wasm not found. Build with: cd examples/plugins/python && extism-py plugin.py -o plugin.wasm");
    }

    // ========================================
    // 4. JavaScript Wasm plugin (extism-js)
    // ========================================
    let js_path = find_plugin_dir().join("javascript").join("plugin.wasm");
    println!("\n--- JavaScript Wasm plugin ---");
    println!("Path: {}", js_path.display());

    // JS plugin has a different name from Python — rename to avoid conflict
    if js_path.exists() {
        let backend = dynplug::WasmBackend::load(&js_path)?;
        // WasmBackend derives name from filename, both are "plugin" — load manually with distinct name
        let manifest = dynplug::extism::Manifest::new([dynplug::extism::Wasm::file(&js_path)]);
        manager.load_wasm_manifest("js_plugin", &manifest, true)?;
        drop(backend);
        println!("[LOADED] js_plugin (wasm/javascript)");
    } else {
        println!("[SKIP] plugin.wasm not found. Build with: cd examples/plugins/javascript && extism-js plugin.js -i plugin.d.ts -o plugin.wasm");
    }

    // ========================================
    // Summary
    // ========================================
    println!("\n--- Loaded Plugins ---");
    for name in manager.names() {
        let kind = manager.plugin_kind(name).unwrap_or("unknown");
        println!("  {name} ({kind})");
    }

    // ========================================
    // Invoke greet on all plugins
    // ========================================
    println!("\n--- Invoke greet ---");

    // Native Rust
    let r = manager.invoke("greeter", "greet", b"World")?;
    let s = std::str::from_utf8(&r)?;
    println!("  greeter:                  {s}");
    assert_eq!(s, "Hello, World!");

    // Rust Wasm
    let r = manager.invoke("dynplug_example_wasm", "greet", b"World")?;
    let s = std::str::from_utf8(&r)?;
    println!("  dynplug_example_wasm:     {s}");
    assert_eq!(s, "Hello, World!");

    // Python
    if manager.plugin_kind("plugin").is_some() {
        let r = manager.invoke("plugin", "greet", b"World")?;
        let s = std::str::from_utf8(&r)?;
        println!("  plugin (python):          {s}");
        assert_eq!(s, "Hello from Python, World!");
    }

    // JavaScript
    if manager.plugin_kind("js_plugin").is_some() {
        let r = manager.invoke("js_plugin", "greet", b"World")?;
        let s = std::str::from_utf8(&r)?;
        println!("  js_plugin (javascript):   {s}");
        assert_eq!(s, "Hello from JavaScript, World!");
    }

    // ========================================
    // Invoke add(21, 21) on all plugins
    // ========================================
    println!("\n--- Invoke add(21, 21) ---");
    let mut add_input = Vec::new();
    add_input.extend_from_slice(&21_i32.to_le_bytes());
    add_input.extend_from_slice(&21_i32.to_le_bytes());

    let r = manager.invoke("greeter", "add", &add_input)?;
    let sum = i32::from_le_bytes(r.try_into().unwrap());
    println!("  greeter:                  {sum}");
    assert_eq!(sum, 42);

    let r = manager.invoke("dynplug_example_wasm", "add", &add_input)?;
    let sum = i32::from_le_bytes(r.try_into().unwrap());
    println!("  dynplug_example_wasm:     {sum}");
    assert_eq!(sum, 42);

    if manager.plugin_kind("plugin").is_some() {
        let r = manager.invoke("plugin", "add", &add_input)?;
        let sum = i32::from_le_bytes(r.try_into().unwrap());
        println!("  plugin (python):          {sum}");
        assert_eq!(sum, 42);
    }

    if manager.plugin_kind("js_plugin").is_some() {
        let r = manager.invoke("js_plugin", "add", &add_input)?;
        let sum = i32::from_le_bytes(r.try_into().unwrap());
        println!("  js_plugin (javascript):   {sum}");
        assert_eq!(sum, 42);
    }

    // ========================================
    // Invoke language() on Python/JS
    // ========================================
    println!("\n--- Invoke language() ---");
    if manager.plugin_kind("plugin").is_some() {
        let r = manager.invoke("plugin", "language", &[])?;
        let s = std::str::from_utf8(&r)?;
        println!("  plugin:      {s}");
        assert_eq!(s, "python");
    }
    if manager.plugin_kind("js_plugin").is_some() {
        let r = manager.invoke("js_plugin", "language", &[])?;
        let s = std::str::from_utf8(&r)?;
        println!("  js_plugin:   {s}");
        assert_eq!(s, "javascript");
    }

    println!("\n=== All checks passed! ===");
    Ok(())
}

#[cfg(feature = "wasm")]
fn find_native_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    let target_dir = exe.parent().unwrap().parent().unwrap();
    target_dir.join(dynplug::lib_filename("dynplug-example"))
}

#[cfg(feature = "wasm")]
fn find_rust_wasm_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    let target_dir = exe.parent().unwrap().parent().unwrap().parent().unwrap();
    target_dir
        .join("wasm32-unknown-unknown")
        .join("debug")
        .join("dynplug_example_wasm.wasm")
}

#[cfg(feature = "wasm")]
fn find_plugin_dir() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("examples")
        .join("plugins")
}
