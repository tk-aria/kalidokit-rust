fn main() {
    // ten-vad's build.rs sets the framework link path, but we also need
    // the @rpath for the test/example binaries in this crate.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" || target_os == "ios" {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let ten_vad_xcfw = std::path::Path::new(&manifest)
            .join("../ten-vad/xcframework/macOS/ten_vad.xcframework");
        if ten_vad_xcfw.exists() {
            // Find the macOS slice
            if let Ok(entries) = std::fs::read_dir(&ten_vad_xcfw) {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().contains("macos") {
                        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", entry.path().display());
                        break;
                    }
                }
            }
        }
    }
}
