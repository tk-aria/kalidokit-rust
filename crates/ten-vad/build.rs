//! build.rs — Prebuilt binary link configuration
//!
//! - Linux/Windows/Android: emit link-search + link-lib only
//! - macOS/iOS: convert upstream .framework to .xcframework via xcodebuild, then link
//! - No C/C++ source compilation

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let vendor_lib = manifest_dir.join("vendor/lib");
    let xcframework_cache = manifest_dir.join("xcframework");

    println!("cargo:rerun-if-changed=vendor/lib");
    println!("cargo:rerun-if-changed=build.rs");

    match target_os.as_str() {
        "linux" => link_linux(&vendor_lib, &target_arch),
        "windows" => link_windows(&vendor_lib, &target_arch),
        "macos" => link_macos(&vendor_lib, &xcframework_cache),
        "ios" => link_ios(&vendor_lib, &xcframework_cache, &target_arch),
        "android" => link_android(&vendor_lib, &target_arch),
        other => panic!("Unsupported target OS: {other}"),
    }
}

fn link_linux(vendor_lib: &Path, arch: &str) {
    let dir = match arch {
        "x86_64" => vendor_lib.join("Linux/x64"),
        other => panic!("Unsupported Linux arch: {other}"),
    };
    assert!(
        dir.join("libten_vad.so").exists(),
        "Missing: {}/libten_vad.so — run: git submodule update --init",
        dir.display()
    );
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=dylib=ten_vad");
}

fn link_windows(vendor_lib: &Path, arch: &str) {
    let subdir = match arch {
        "x86_64" => "x64",
        "x86" => "x86",
        other => panic!("Unsupported Windows arch: {other}"),
    };
    let dir = vendor_lib.join(format!("Windows/{subdir}"));
    assert!(
        dir.join("ten_vad.lib").exists(),
        "Missing: {}/ten_vad.lib",
        dir.display()
    );
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=dylib=ten_vad");
}

fn link_macos(vendor_lib: &Path, cache: &Path) {
    let xcfw = cache.join("macOS/ten_vad.xcframework");

    if !xcfw.join("Info.plist").exists() {
        let src = vendor_lib.join("macOS/ten_vad.framework");
        assert!(src.exists(), "Missing: {}", src.display());
        create_xcframework(&[&src], &xcfw);
    }

    let slice = find_slice(&xcfw, "macos");
    println!("cargo:rustc-link-search=framework={}", slice.display());
    println!("cargo:rustc-link-lib=framework=ten_vad");
    // Set @rpath so dyld can find the framework at runtime
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", slice.display());
}

fn link_ios(vendor_lib: &Path, cache: &Path, arch: &str) {
    let xcfw = cache.join("iOS/ten_vad.xcframework");

    if !xcfw.join("Info.plist").exists() {
        let device_fw = vendor_lib.join("iOS/ten_vad.framework");
        assert!(device_fw.exists(), "Missing: {}", device_fw.display());

        let sim_fw = vendor_lib.join("iOS-simulator/ten_vad.framework");
        if sim_fw.exists() {
            create_xcframework(&[&device_fw, &sim_fw], &xcfw);
        } else {
            eprintln!(
                "cargo:warning=iOS simulator framework not found, \
                 creating device-only xcframework"
            );
            create_xcframework(&[&device_fw], &xcfw);
        }
    }

    let is_sim = env::var("TARGET").unwrap_or_default().contains("sim") || arch == "x86_64";
    let keyword = if is_sim { "simulator" } else { "ios-arm64" };
    let slice = find_slice(&xcfw, keyword);
    println!("cargo:rustc-link-search=framework={}", slice.display());
    println!("cargo:rustc-link-lib=framework=ten_vad");
}

fn link_android(vendor_lib: &Path, arch: &str) {
    let abi = match arch {
        "aarch64" => "arm64-v8a",
        "arm" => "armeabi-v7a",
        other => panic!("Unsupported Android arch: {other}"),
    };
    let dir = vendor_lib.join(format!("Android/{abi}"));
    assert!(
        dir.join("libten_vad.so").exists(),
        "Missing: {}/libten_vad.so",
        dir.display()
    );
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=dylib=ten_vad");
}

/// Run `xcodebuild -create-xcframework` with the given input frameworks.
fn create_xcframework(frameworks: &[&Path], output: &Path) {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    if output.exists() {
        std::fs::remove_dir_all(output).unwrap();
    }

    let mut cmd = Command::new("xcodebuild");
    cmd.arg("-create-xcframework");
    for fw in frameworks {
        cmd.arg("-framework").arg(fw);
    }
    cmd.arg("-output").arg(output);

    let status = cmd
        .status()
        .expect("xcodebuild not found — install Xcode CLI tools");
    assert!(
        status.success(),
        "xcodebuild -create-xcframework failed (exit {status})"
    );
    eprintln!("cargo:warning=Created xcframework: {}", output.display());
}

/// Find a directory inside the xcframework whose name contains `keyword`.
fn find_slice(xcfw: &Path, keyword: &str) -> PathBuf {
    for entry in std::fs::read_dir(xcfw)
        .unwrap_or_else(|_| panic!("Cannot read {}", xcfw.display()))
        .flatten()
    {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if entry.file_name().to_string_lossy().contains(keyword) {
            return entry.path();
        }
    }
    panic!("No slice matching '{keyword}' in {}", xcfw.display());
}
