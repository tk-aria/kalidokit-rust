//! Example host binary demonstrating Wasm plugin loading via Extism.
//!
//! Prerequisites:
//!   cargo build -p dynplug-example-wasm --target wasm32-unknown-unknown
//!
//! Run:
//!   cargo run -p dynplug --features wasm --example host_wasm

#[cfg(not(feature = "wasm"))]
fn main() {
    eprintln!("This example requires the `wasm` feature.");
    eprintln!("Run: cargo run -p dynplug --features wasm --example host_wasm");
    std::process::exit(1);
}

#[cfg(feature = "wasm")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use dynplug::PluginBackend;

    let wasm_path = find_wasm_path();
    println!("=== dynplug Wasm Host Example ===");
    println!("Wasm plugin path: {}", wasm_path.display());

    // ========================================
    // WasmBackend direct usage
    // ========================================
    println!("\n--- WasmBackend Direct ---");
    {
        let mut plugin = dynplug::WasmBackend::load(&wasm_path)?;
        println!("Plugin name: {}", plugin.name());
        println!("Plugin kind: {}", plugin.kind());

        // greet
        let result = plugin.invoke("greet", b"World")?;
        let greeting = std::str::from_utf8(&result)?;
        println!("greet(\"World\") = {greeting}");
        assert_eq!(greeting, "Hello, World!");
        println!("[PASS] greet");

        // add
        let mut input = Vec::new();
        input.extend_from_slice(&21_i32.to_le_bytes());
        input.extend_from_slice(&21_i32.to_le_bytes());
        let result = plugin.invoke("add", &input)?;
        let sum = i32::from_le_bytes(result.try_into().unwrap());
        println!("add(21, 21) = {sum}");
        assert_eq!(sum, 42);
        println!("[PASS] add");

        // noop
        let result = plugin.invoke("noop", &[])?;
        assert!(result.is_empty());
        println!("[PASS] noop (empty output)");

        // unknown method -> error
        let err = plugin.invoke("nonexistent", &[]);
        assert!(err.is_err());
        println!("[PASS] unknown method returns error");
    }

    // ========================================
    // PluginManager with Wasm
    // ========================================
    println!("\n--- PluginManager (Wasm) ---");
    {
        let mut manager = dynplug::PluginManager::new();

        let name = manager.load_wasm(&wasm_path)?.to_string();
        println!("Loaded wasm plugin: {name}");
        assert_eq!(manager.plugin_kind(&name), Some("wasm"));
        println!("[PASS] load_wasm");

        // Invoke via manager
        let result = manager.invoke(&name, "greet", b"PluginManager")?;
        let greeting = std::str::from_utf8(&result)?;
        println!("invoke greet = {greeting}");
        assert_eq!(greeting, "Hello, PluginManager!");
        println!("[PASS] invoke via manager");

        // Unload
        manager.unload(&name)?;
        assert!(manager.names().is_empty());
        println!("[PASS] unload");
    }

    // ========================================
    // Mixed: Native + Wasm
    // ========================================
    println!("\n--- Mixed: Native + Wasm ---");
    {
        let native_path = find_native_path();
        let mut manager = dynplug::PluginManager::new();

        manager.load_file(&native_path)?;
        manager.load_wasm(&wasm_path)?;

        println!("Loaded plugins: {:?}", manager.names());
        assert_eq!(manager.plugin_kind("greeter"), Some("native"));
        assert_eq!(manager.plugin_kind("dynplug_example_wasm"), Some("wasm"));

        let r1 = manager.invoke("greeter", "greet", b"Native")?;
        println!("native greet = {}", std::str::from_utf8(&r1)?);
        assert_eq!(std::str::from_utf8(&r1)?, "Hello, Native!");

        let r2 = manager.invoke("dynplug_example_wasm", "greet", b"Wasm")?;
        println!("wasm greet = {}", std::str::from_utf8(&r2)?);
        assert_eq!(std::str::from_utf8(&r2)?, "Hello, Wasm!");

        println!("[PASS] mixed native + wasm");
    }

    println!("\n=== All checks passed! ===");
    Ok(())
}

#[cfg(feature = "wasm")]
fn find_wasm_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    // examples binary is in target/debug/examples/host_wasm
    // wasm is in target/wasm32-unknown-unknown/debug/
    let target_dir = exe.parent().unwrap().parent().unwrap().parent().unwrap();
    target_dir
        .join("wasm32-unknown-unknown")
        .join("debug")
        .join("dynplug_example_wasm.wasm")
}

#[cfg(feature = "wasm")]
fn find_native_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    let target_dir = exe.parent().unwrap().parent().unwrap();
    target_dir.join(dynplug::lib_filename("dynplug-example"))
}
