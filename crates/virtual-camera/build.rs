fn main() {
    // Only compile Objective-C extension code on macOS
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=macos-extension/");

        cc::Build::new()
            .files(&[
                "macos-extension/ProviderSource.m",
                "macos-extension/DeviceSource.m",
                "macos-extension/StreamSource.m",
                "macos-extension/SinkStreamSource.m",
            ])
            .flag("-fobjc-arc")
            .flag("-fmodules")
            .include("macos-extension")
            .compile("vcam_extension");

        // Link required frameworks
        println!("cargo:rustc-link-lib=framework=CoreMediaIO");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
}
