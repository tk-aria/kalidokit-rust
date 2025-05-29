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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lib_extension_current_os() {
        let ext = lib_extension();
        #[cfg(target_os = "macos")]
        assert_eq!(ext, "dylib");
        #[cfg(target_os = "linux")]
        assert_eq!(ext, "so");
        #[cfg(target_os = "windows")]
        assert_eq!(ext, "dll");
    }

    #[test]
    fn test_lib_prefix_current_os() {
        let prefix = lib_prefix();
        #[cfg(target_os = "windows")]
        assert_eq!(prefix, "");
        #[cfg(not(target_os = "windows"))]
        assert_eq!(prefix, "lib");
    }

    #[test]
    fn test_lib_filename_with_hyphen() {
        let name = lib_filename("my-plugin");
        #[cfg(target_os = "macos")]
        assert_eq!(name, "libmy_plugin.dylib");
        #[cfg(target_os = "linux")]
        assert_eq!(name, "libmy_plugin.so");
        #[cfg(target_os = "windows")]
        assert_eq!(name, "my_plugin.dll");
    }

    #[test]
    fn test_lib_filename_no_hyphen() {
        let name = lib_filename("simple");
        #[cfg(target_os = "macos")]
        assert_eq!(name, "libsimple.dylib");
        #[cfg(target_os = "linux")]
        assert_eq!(name, "libsimple.so");
        #[cfg(target_os = "windows")]
        assert_eq!(name, "simple.dll");
    }

    #[test]
    fn test_lib_filename_empty_string() {
        // Should not panic
        let name = lib_filename("");
        assert!(!name.is_empty()); // at least has prefix and extension
    }
}
