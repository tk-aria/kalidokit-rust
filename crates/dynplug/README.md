# dynplug

> Cross-platform dynamic plugin loading for Rust

## Features

- **3-layer abstraction** -- Choose the right level for your use case:
  - **Layer 1 (Symbol Bind)**: Bind individual symbols from any shared library
  - **Layer 2 (VTable)**: Structured `#[repr(C)]` function pointer tables with version checking
  - **Layer 3 (define_plugin!)**: Auto-generated VTable + safe wrapper from a trait-like definition
- **PluginManager** -- Centralized lifecycle management with guaranteed cleanup
- **Cross-platform** -- Linux, macOS, Windows, Android
- **Panic safety** -- `export_plugin!` catches panics at FFI boundaries

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
dynplug = { version = "0.1", path = "crates/dynplug" }
```

## Quick Start

### Creating a plugin (cdylib)

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
dynplug = { version = "0.1", path = "../dynplug" }
```

```rust
// src/lib.rs
dynplug::export_plugin! {
    name: "greeter",
    version: 1,
    invoke: handle_invoke,
}

fn handle_invoke(method: &str, input: &[u8]) -> Result<Vec<u8>, String> {
    match method {
        "greet" => {
            let name = std::str::from_utf8(input).unwrap_or("world");
            Ok(format!("Hello, {name}!").into_bytes())
        }
        _ => Err(format!("unknown method: {method}")),
    }
}
```

### Loading a plugin (host)

```rust
use dynplug::{LoadedLibrary, PluginManager};

// Layer 2: VTable
let lib = LoadedLibrary::load("path/to/plugin.dylib")?;
let vt = lib.vtable::<dynplug::PluginVTable>(None)?;

// Layer 3: define_plugin!
dynplug::define_plugin! {
    pub struct Calculator {
        fn add(a: i32, b: i32) -> i32;
    }
}
let calc = Calculator::load("path/to/calculator.dylib")?;
assert_eq!(calc.add(21, 21), 42);

// PluginManager
let mut manager = PluginManager::new();
manager.load_file("path/to/plugin.dylib")?;
manager.load_from_directory("plugins/")?;
let plugin = manager.get("greeter").unwrap();
```

## API Layers

### Layer 1: Symbol Bind

Bind individual symbols with lifetime safety:

```rust
let lib = LoadedLibrary::load("libfoo.so")?;
let add = lib.bind::<extern "C" fn(i32, i32) -> i32>("add")?;
let result = add(21, 21); // Deref makes this work naturally
```

### Layer 2: VTable

Load a `#[repr(C)]` VTable with version checking:

```rust
let lib = LoadedLibrary::load("plugin.dylib")?;
let vt = lib.vtable::<PluginVTable>(None)?;
// vt.invoke, vt.name, vt.version, vt.free_buffer, vt.destroy
```

The standard `PluginVTable` provides a generic invoke interface:

| Return code | Meaning | `out_ptr` / `out_len` |
|:-----------:|---------|----------------------|
| `0` | Success | Output buffer (call `free_buffer` after use) |
| `-1` | Application error | UTF-8 error message (call `free_buffer`) |
| `-2` | Plugin panicked | Undefined (do NOT call `free_buffer`) |

### Layer 3: define_plugin!

Auto-generate VTable and safe wrapper (v0.1: primitives only):

```rust
dynplug::define_plugin! {
    pub struct MyPlugin {
        fn compute(x: f64, y: f64) -> f64;
    }
}

// Generates:
// - MyPluginVTable: #[repr(C)] struct with function pointers + destroy
// - MyPlugin: wrapper with load(path) and safe method calls
// - VTableValidate impl for MyPluginVTable
// - Drop impl that calls destroy()
```

### PluginManager

Centralized lifecycle for multiple plugins:

```rust
let mut manager = PluginManager::new();

// Load from files and directories
manager.load_file("path/to/plugin.dylib")?;
manager.load_from_directory("plugins/")?;
manager.load_paths(["plugins/", "extra/libfoo.so"])?;

// Lookup and list
let lib = manager.get("greeter").unwrap();
let names = manager.names();

// Unload individually or all at once
manager.unload("greeter")?;
manager.unload_all(); // also called on Drop (reverse order)
```

## Building

```bash
# Build the library
cargo build -p dynplug

# Build example plugin
cargo build -p dynplug-example

# Run host example
cargo build -p dynplug-example && cargo run -p dynplug --example host

# Run tests
cargo build -p dynplug-example && cargo test -p dynplug

# Lint
cargo clippy -p dynplug -- -D warnings
```

## Platform Support

| Platform | Extension | Status |
|----------|-----------|--------|
| Linux x86_64 / aarch64 | `.so` | Supported |
| macOS x86_64 / aarch64 | `.dylib` | Supported |
| Windows x86_64 | `.dll` | Supported |
| Android aarch64 / armv7 | `.so` | Supported |
| iOS | -- | Unsupported (Apple policy) |

## License

See workspace root LICENSE.
