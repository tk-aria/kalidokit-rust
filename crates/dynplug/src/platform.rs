//! Platform-specific utilities for dynamic library file naming.

/// Returns the shared library file extension (without dot) for the current platform.
///
/// - Linux/Android: "so"
/// - macOS: "dylib"
/// - Windows: "dll"
pub fn lib_extension() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so" // linux, android, freebsd, etc.
    }
}

/// Returns the shared library filename prefix for the current platform.
///
/// - Windows: "" (no prefix)
/// - Others: "lib"
pub fn lib_prefix() -> &'static str {
    if cfg!(target_os = "windows") {
        ""
    } else {
        "lib"
    }
}

/// Converts a Cargo crate name to the platform-specific library filename.
///
/// Cargo converts hyphens to underscores in output filenames.
///
/// # Examples
/// - `"dynplug-example"` → `"libdynplug_example.dylib"` (macOS)
/// - `"dynplug-example"` → `"libdynplug_example.so"` (Linux)
/// - `"dynplug-example"` → `"dynplug_example.dll"` (Windows)
pub fn lib_filename(crate_name: &str) -> String {
    let name = crate_name.replace('-', "_");
    format!("{}{name}.{}", lib_prefix(), lib_extension())
}
