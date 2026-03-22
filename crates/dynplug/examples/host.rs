//! Example host binary demonstrating all dynplug layers.

use dynplug::{LoadedLibrary, PluginManager};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lib_path = find_plugin_path();
    println!("=== dynplug Host Example ===");
    println!("Plugin path: {}", lib_path.display());

    // ========================================
    // Layer 1: Symbol Bind
    // ========================================
    println!("\n--- Layer 1: Symbol Bind ---");
    {
        let lib = LoadedLibrary::load(&lib_path)?;
        let entry = lib.bind::<dynplug::PluginEntryFn>("plugin_entry")?;
        let vtable = unsafe { &*entry() };
        let name = unsafe { std::ffi::CStr::from_ptr((vtable.name)()).to_str().unwrap() };
        println!("Plugin name (via bind): {name}");
        println!("Plugin version: {}", (vtable.version)());
        assert_eq!(name, "greeter");
        println!("[PASS] Layer 1: Symbol Bind");
    }

    // ========================================
    // Layer 2: VTable
    // ========================================
    println!("\n--- Layer 2: VTable ---");
    {
        let lib = LoadedLibrary::load(&lib_path)?;
        let vt = lib.vtable::<dynplug::PluginVTable>(None)?;

        // invoke: greet
        let mut out_ptr: *mut u8 = std::ptr::null_mut();
        let mut out_len: usize = 0;
        let rc = (vt.invoke)(
            b"greet".as_ptr(),
            5,
            b"World".as_ptr(),
            5,
            &mut out_ptr,
            &mut out_len,
        );
        assert_eq!(rc, 0);
        let greeting = unsafe {
            String::from_utf8_lossy(std::slice::from_raw_parts(out_ptr, out_len)).to_string()
        };
        (vt.free_buffer)(out_ptr, out_len);
        println!("greet(\"World\") = {greeting}");
        assert_eq!(greeting, "Hello, World!");
        println!("[PASS] invoke greet");

        // invoke: add
        let mut add_input = Vec::new();
        add_input.extend_from_slice(&21_i32.to_le_bytes());
        add_input.extend_from_slice(&21_i32.to_le_bytes());
        let rc = (vt.invoke)(
            b"add".as_ptr(),
            3,
            add_input.as_ptr(),
            add_input.len(),
            &mut out_ptr,
            &mut out_len,
        );
        assert_eq!(rc, 0);
        let sum = i32::from_le_bytes(
            unsafe { std::slice::from_raw_parts(out_ptr, out_len) }
                .try_into()
                .unwrap(),
        );
        (vt.free_buffer)(out_ptr, out_len);
        println!("add(21, 21) = {sum}");
        assert_eq!(sum, 42);
        println!("[PASS] invoke add");

        // invoke: unknown method -> error (-1)
        let rc = (vt.invoke)(
            b"unknown".as_ptr(),
            7,
            std::ptr::null(),
            0,
            &mut out_ptr,
            &mut out_len,
        );
        assert_eq!(rc, -1);
        let err_msg = unsafe {
            String::from_utf8_lossy(std::slice::from_raw_parts(out_ptr, out_len)).to_string()
        };
        (vt.free_buffer)(out_ptr, out_len);
        println!("unknown -> error: {err_msg}");
        println!("[PASS] invoke unknown method returns -1");

        // invoke: panic -> -2
        let rc = (vt.invoke)(
            b"panic_test".as_ptr(),
            10,
            std::ptr::null(),
            0,
            &mut out_ptr,
            &mut out_len,
        );
        assert_eq!(rc, -2);
        println!("panic_test -> caught (rc={rc})");
        println!("[PASS] invoke panic returns -2");

        (vt.destroy)();
    }

    // ========================================
    // PluginManager
    // ========================================
    println!("\n--- PluginManager ---");
    {
        let mut manager = PluginManager::new();

        // Load file
        manager.load_file(&lib_path)?;
        println!("Loaded plugins: {:?}", manager.names());
        println!("[PASS] load_file");

        // Get by name
        let p = manager.get("greeter").expect("greeter not found");
        println!("get(\"greeter\"): path={}", p.path().display());
        println!("[PASS] get");

        // Unload
        manager.unload("greeter")?;
        assert!(manager.get("greeter").is_none());
        println!("[PASS] unload");

        // Directory scan
        let plugin_dir = lib_path.parent().unwrap();
        let count = manager.load_from_directory(plugin_dir)?;
        println!("load_from_directory: {count} plugin(s)");
        assert!(count >= 1);
        println!("[PASS] load_from_directory");

        // Drop releases all
    }
    println!("[PASS] PluginManager dropped (all plugins released)");

    println!("\n=== All checks passed! ===");
    Ok(())
}

fn find_plugin_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let target_dir = exe.parent().unwrap().parent().unwrap();
    target_dir.join(dynplug::lib_filename("dynplug-example"))
}
