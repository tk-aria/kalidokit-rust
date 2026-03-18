# ten-vad-rs — Rust バインディング設計

## 1. 概要

[TEN VAD](https://github.com/TEN-framework/ten-vad) (Voice Activity Detector) の Rust バインディングクレート。
低レイテンシ・高性能・軽量な音声アクティビティ検出を Rust から利用可能にする。

## 2. TEN VAD C API

```c
// include/ten_vad.h — 全4関数
typedef void *ten_vad_handle_t;

int ten_vad_create(ten_vad_handle_t *handle, size_t hop_size, float threshold);
int ten_vad_process(ten_vad_handle_t handle, const int16_t *audio_data,
                    size_t audio_data_length, float *out_probability, int *out_flag);
int ten_vad_destroy(ten_vad_handle_t *handle);
const char *ten_vad_get_version(void);
```

- **入力**: 16kHz 16bit PCM, フレームサイズ 160 or 256 サンプル
- **出力**: 確率 [0.0, 1.0] + バイナリフラグ (0/1)

## 3. 配布ライブラリ形式

| プラットフォーム | 配布形式 | アーキテクチャ | 上流提供形式 |
|---|---|---|---|
| Linux | `.so` | x86_64 | `.so` (そのまま) |
| Windows | `.dll` + `.lib` | x86_64, x86 | `.dll` + `.lib` (そのまま) |
| macOS | `.xcframework` | arm64 + x86_64 | `.framework` → build.rs で変換 |
| iOS | `.xcframework` | arm64 (device) + x86_64 (sim) | `.framework` → build.rs で変換 |
| Android | `.so` | arm64, armv7 | `.so` (そのまま) |

## 4. クレート構成

```
crates/ten-vad/
├── Cargo.toml
├── build.rs                    # ライブラリ変換 + リンク設定 (全処理をここに集約)
├── src/
│   ├── lib.rs                  # 安全な Rust API (TenVad, VadResult, VadError)
│   └── ffi.rs                  # 手書き FFI 宣言 (4 関数)
├── vendor/                     # git submodule (TEN-framework/ten-vad)
│   ├── include/ten_vad.h
│   ├── lib/
│   │   ├── Linux/x64/libten_vad.so
│   │   ├── Windows/x64/ten_vad.dll + ten_vad.lib
│   │   ├── Windows/x86/ten_vad.dll + ten_vad.lib
│   │   ├── macOS/ten_vad.framework/          ← 上流提供
│   │   ├── iOS/ten_vad.framework/            ← 上流提供
│   │   └── Android/
│   │       ├── arm64-v8a/libten_vad.so
│   │       └── armeabi-v7a/libten_vad.so
│   └── src/                    # C++ ソース (iOS simulator ビルド用)
├── xcframework/                # build.rs が生成する .xcframework (gitignore)
│   ├── macOS/ten_vad.xcframework/
│   └── iOS/ten_vad.xcframework/
└── examples/
    └── detect_vad.rs
```

## 5. build.rs — 全処理を集約

```rust
//! build.rs
//!
//! 1. Apple プラットフォームでは上流の .framework を .xcframework に変換
//! 2. プラットフォーム別のリンカフラグを出力
//!
//! .xcframework 変換は xcodebuild コマンドを呼び出す (macOS ビルドホスト必須)。
//! 変換済みの xcframework はキャッシュされ、2回目以降はスキップする。

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let vendor_lib = manifest_dir.join("vendor/lib");
    let xcframework_dir = manifest_dir.join("xcframework");

    // リビルドトリガー: vendor ディレクトリが変更されたら再実行
    println!("cargo:rerun-if-changed=vendor/lib");
    println!("cargo:rerun-if-changed=build.rs");

    match target_os.as_str() {
        "linux" => link_linux(&vendor_lib, &target_arch),
        "windows" => link_windows(&vendor_lib, &target_arch),
        "macos" => link_macos(&vendor_lib, &xcframework_dir),
        "ios" => link_ios(&vendor_lib, &xcframework_dir, &target_arch),
        "android" => link_android(&vendor_lib, &target_arch),
        _ => panic!("Unsupported target OS: {}", target_os),
    }
}

// ============================================================
// Linux
// ============================================================
fn link_linux(vendor_lib: &Path, target_arch: &str) {
    let lib_dir = match target_arch {
        "x86_64" => vendor_lib.join("Linux/x64"),
        _ => panic!("Unsupported Linux arch: {}", target_arch),
    };
    assert!(lib_dir.join("libten_vad.so").exists(),
        "Missing: {}/libten_vad.so", lib_dir.display());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=ten_vad");
}

// ============================================================
// Windows
// ============================================================
fn link_windows(vendor_lib: &Path, target_arch: &str) {
    let arch_dir = match target_arch {
        "x86_64" => "x64",
        "x86" => "x86",
        _ => panic!("Unsupported Windows arch: {}", target_arch),
    };
    let lib_dir = vendor_lib.join(format!("Windows/{}", arch_dir));
    assert!(lib_dir.join("ten_vad.lib").exists(),
        "Missing: {}/ten_vad.lib", lib_dir.display());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=ten_vad");
}

// ============================================================
// macOS — .framework → .xcframework 変換 + リンク
// ============================================================
fn link_macos(vendor_lib: &Path, xcframework_dir: &Path) {
    let xcfw_path = xcframework_dir.join("macOS/ten_vad.xcframework");
    let slice_dir = xcfw_path.join("macos-arm64_x86_64");

    if !slice_dir.join("ten_vad.framework").exists() {
        // .xcframework がまだ存在しない → build.rs 内で xcodebuild を実行して変換
        let src_fw = vendor_lib.join("macOS/ten_vad.framework");
        assert!(src_fw.exists(), "Missing upstream framework: {}", src_fw.display());

        std::fs::create_dir_all(xcframework_dir.join("macOS")).unwrap();

        // 古い xcframework があれば削除 (xcodebuild は上書き不可)
        if xcfw_path.exists() {
            std::fs::remove_dir_all(&xcfw_path).unwrap();
        }

        let status = Command::new("xcodebuild")
            .args([
                "-create-xcframework",
                "-framework", &src_fw.to_string_lossy(),
                "-output", &xcfw_path.to_string_lossy(),
            ])
            .status()
            .expect("Failed to execute xcodebuild. Is Xcode CLI tools installed?");

        assert!(status.success(), "xcodebuild -create-xcframework failed for macOS");
        eprintln!("cargo:warning=Created macOS xcframework at {}", xcfw_path.display());
    }

    // xcframework 内の framework を link search path に追加
    println!("cargo:rustc-link-search=framework={}", slice_dir.display());
    println!("cargo:rustc-link-lib=framework=ten_vad");
}

// ============================================================
// iOS — .framework → .xcframework 変換 (device + simulator) + リンク
// ============================================================
fn link_ios(vendor_lib: &Path, xcframework_dir: &Path, target_arch: &str) {
    let xcfw_path = xcframework_dir.join("iOS/ten_vad.xcframework");

    if !xcfw_path.join("Info.plist").exists() {
        // .xcframework がまだ存在しない → build.rs 内で生成
        let device_fw = vendor_lib.join("iOS/ten_vad.framework");
        assert!(device_fw.exists(), "Missing upstream iOS framework: {}", device_fw.display());

        std::fs::create_dir_all(xcframework_dir.join("iOS")).unwrap();

        if xcfw_path.exists() {
            std::fs::remove_dir_all(&xcfw_path).unwrap();
        }

        // iOS simulator 用 framework の存在チェック
        // 上流が提供していない場合は C++ ソースからクロスコンパイルを試みる
        let sim_fw = vendor_lib.join("iOS-simulator/ten_vad.framework");
        if sim_fw.exists() {
            // device + simulator の両方を xcframework に統合
            let status = Command::new("xcodebuild")
                .args([
                    "-create-xcframework",
                    "-framework", &device_fw.to_string_lossy(),
                    "-framework", &sim_fw.to_string_lossy(),
                    "-output", &xcfw_path.to_string_lossy(),
                ])
                .status()
                .expect("Failed to execute xcodebuild");

            assert!(status.success(), "xcodebuild -create-xcframework failed for iOS");
            eprintln!("cargo:warning=Created iOS xcframework (device + simulator)");
        } else {
            // simulator framework がない → device のみで xcframework 作成
            eprintln!("cargo:warning=iOS simulator framework not found, creating device-only xcframework");
            let status = Command::new("xcodebuild")
                .args([
                    "-create-xcframework",
                    "-framework", &device_fw.to_string_lossy(),
                    "-output", &xcfw_path.to_string_lossy(),
                ])
                .status()
                .expect("Failed to execute xcodebuild");

            assert!(status.success(), "xcodebuild -create-xcframework failed for iOS (device-only)");
        }
    }

    // ターゲットに応じた xcframework slice を選択
    let is_simulator = env::var("TARGET").unwrap_or_default().contains("sim")
        || target_arch == "x86_64"; // iOS x86_64 は常に simulator
    let slice = if is_simulator {
        "ios-arm64_x86_64-simulator"
    } else {
        "ios-arm64"
    };

    let slice_dir = xcfw_path.join(slice);
    if !slice_dir.exists() {
        // フォールバック: slice 名が異なる場合 (xcodebuild が自動命名)
        // xcframework 内のディレクトリを走査して最初にマッチするものを使用
        let fallback = find_xcframework_slice(&xcfw_path, if is_simulator { "simulator" } else { "ios-arm64" });
        println!("cargo:rustc-link-search=framework={}", fallback.display());
    } else {
        println!("cargo:rustc-link-search=framework={}", slice_dir.display());
    }
    println!("cargo:rustc-link-lib=framework=ten_vad");
}

/// xcframework 内のディレクトリから pattern にマッチする slice を探す
fn find_xcframework_slice(xcfw_path: &Path, pattern: &str) -> PathBuf {
    if let Ok(entries) = std::fs::read_dir(xcfw_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains(pattern) && entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                return entry.path();
            }
        }
    }
    panic!("No xcframework slice matching '{}' found in {}", pattern, xcfw_path.display());
}

// ============================================================
// Android
// ============================================================
fn link_android(vendor_lib: &Path, target_arch: &str) {
    let abi_dir = match target_arch {
        "aarch64" => "arm64-v8a",
        "arm" => "armeabi-v7a",
        _ => panic!("Unsupported Android arch: {}", target_arch),
    };
    let lib_dir = vendor_lib.join(format!("Android/{}", abi_dir));
    assert!(lib_dir.join("libten_vad.so").exists(),
        "Missing: {}/libten_vad.so", lib_dir.display());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=ten_vad");
}
```

## 6. 手書き FFI — `src/ffi.rs`

```rust
//! Raw FFI bindings to the TEN VAD C library.
//!
//! These declarations correspond exactly to the functions in `include/ten_vad.h`.
//! The library is linked by `build.rs` at compile time.

use std::ffi::c_void;
use std::os::raw::{c_char, c_float, c_int};

/// Opaque handle for a TEN VAD instance.
///
/// Internally allocated by `ten_vad_create` and freed by `ten_vad_destroy`.
pub type TenVadHandle = *mut c_void;

extern "C" {
    /// Create and initialize a TEN VAD instance.
    ///
    /// # Parameters
    /// - `handle`: Pointer to receive the allocated handle.
    /// - `hop_size`: Number of samples per analysis frame (160 or 256 for 16 kHz audio).
    /// - `threshold`: Detection threshold in `[0.0, 1.0]`.
    ///
    /// # Returns
    /// `0` on success, `-1` on failure.
    pub fn ten_vad_create(
        handle: *mut TenVadHandle,
        hop_size: usize,
        threshold: c_float,
    ) -> c_int;

    /// Process one audio frame and produce a voice-activity decision.
    ///
    /// # Parameters
    /// - `handle`: A valid handle returned by `ten_vad_create`.
    /// - `audio_data`: Pointer to `hop_size` samples of 16-bit PCM audio at 16 kHz.
    /// - `audio_data_length`: Must equal `hop_size`.
    /// - `out_probability`: Receives voice probability in `[0.0, 1.0]`.
    /// - `out_flag`: Receives `1` if voice detected (probability ≥ threshold), else `0`.
    ///
    /// # Returns
    /// `0` on success, `-1` on failure.
    pub fn ten_vad_process(
        handle: TenVadHandle,
        audio_data: *const i16,
        audio_data_length: usize,
        out_probability: *mut c_float,
        out_flag: *mut c_int,
    ) -> c_int;

    /// Destroy a TEN VAD instance and release all associated resources.
    ///
    /// Sets `*handle` to `NULL` on success.
    ///
    /// # Returns
    /// `0` on success, `-1` on failure.
    pub fn ten_vad_destroy(handle: *mut TenVadHandle) -> c_int;

    /// Return the library version string (e.g. `"1.0.0"`).
    ///
    /// The returned pointer is valid for the lifetime of the process.
    pub fn ten_vad_get_version() -> *const c_char;
}
```

## 7. 安全な Rust API — `src/lib.rs`

```rust
//! # ten-vad
//!
//! Rust bindings for the [TEN VAD](https://github.com/TEN-framework/ten-vad)
//! voice activity detection library.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ten_vad::{TenVad, HopSize};
//!
//! let mut vad = TenVad::new(HopSize::Samples256, 0.5).unwrap();
//! println!("TEN VAD version: {}", TenVad::version());
//!
//! let audio_frame: Vec<i16> = vec![0i16; 256]; // 16ms of silence at 16kHz
//! let result = vad.process(&audio_frame).unwrap();
//! println!("voice={}, probability={:.3}", result.is_voice, result.probability);
//! // TenVad is automatically destroyed on drop
//! ```

mod ffi;

use std::fmt;

/// Supported hop (frame) sizes for 16 kHz audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopSize {
    /// 160 samples = 10 ms at 16 kHz.
    Samples160 = 160,
    /// 256 samples = 16 ms at 16 kHz.
    Samples256 = 256,
}

impl HopSize {
    /// Number of samples per frame.
    pub fn as_usize(self) -> usize {
        self as usize
    }
}

/// Voice activity detection result for a single audio frame.
#[derive(Debug, Clone, Copy)]
pub struct VadResult {
    /// Voice activity probability in `[0.0, 1.0]`.
    pub probability: f32,
    /// `true` if voice was detected (probability ≥ threshold).
    pub is_voice: bool,
}

/// Errors that can occur during VAD operations.
#[derive(Debug)]
pub enum VadError {
    /// `ten_vad_create` returned an error.
    CreateFailed,
    /// `ten_vad_process` returned an error.
    ProcessFailed,
    /// The audio frame length does not match the configured hop size.
    InvalidFrameSize {
        expected: usize,
        actual: usize,
    },
    /// The threshold is outside the valid range `[0.0, 1.0]`.
    InvalidThreshold(f32),
}

impl fmt::Display for VadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VadError::CreateFailed => write!(f, "ten_vad_create failed"),
            VadError::ProcessFailed => write!(f, "ten_vad_process failed"),
            VadError::InvalidFrameSize { expected, actual } => {
                write!(f, "frame size mismatch: expected {expected}, got {actual}")
            }
            VadError::InvalidThreshold(t) => {
                write!(f, "threshold {t} is outside [0.0, 1.0]")
            }
        }
    }
}

impl std::error::Error for VadError {}

/// A TEN VAD voice activity detector instance.
///
/// Wraps the native C library handle with a safe Rust interface.
/// The handle is automatically destroyed when the `TenVad` is dropped.
///
/// # Thread Safety
///
/// A single `TenVad` instance must not be shared across threads simultaneously
/// (it is `Send` but not `Sync`). Create separate instances for concurrent use.
pub struct TenVad {
    handle: ffi::TenVadHandle,
    hop_size: HopSize,
}

impl TenVad {
    /// Create a new VAD instance.
    ///
    /// # Parameters
    /// - `hop_size`: Frame size — `Samples160` (10 ms) or `Samples256` (16 ms).
    /// - `threshold`: Detection sensitivity in `[0.0, 1.0]`.
    ///   Lower values detect more aggressively (more false positives);
    ///   higher values are more conservative.
    ///
    /// # Errors
    /// - [`VadError::InvalidThreshold`] if threshold is outside `[0.0, 1.0]`.
    /// - [`VadError::CreateFailed`] if the native library fails to initialize.
    pub fn new(hop_size: HopSize, threshold: f32) -> Result<Self, VadError> {
        if !(0.0..=1.0).contains(&threshold) {
            return Err(VadError::InvalidThreshold(threshold));
        }
        let mut handle: ffi::TenVadHandle = std::ptr::null_mut();
        let ret = unsafe {
            ffi::ten_vad_create(&mut handle, hop_size.as_usize(), threshold)
        };
        if ret != 0 || handle.is_null() {
            return Err(VadError::CreateFailed);
        }
        Ok(Self { handle, hop_size })
    }

    /// Process one audio frame and return the VAD decision.
    ///
    /// # Parameters
    /// - `audio_data`: Exactly `hop_size` samples of 16-bit PCM audio at 16 kHz.
    ///
    /// # Errors
    /// - [`VadError::InvalidFrameSize`] if `audio_data.len() != hop_size`.
    /// - [`VadError::ProcessFailed`] if the native library returns an error.
    pub fn process(&mut self, audio_data: &[i16]) -> Result<VadResult, VadError> {
        let expected = self.hop_size.as_usize();
        if audio_data.len() != expected {
            return Err(VadError::InvalidFrameSize {
                expected,
                actual: audio_data.len(),
            });
        }
        let mut probability: f32 = 0.0;
        let mut flag: i32 = 0;
        let ret = unsafe {
            ffi::ten_vad_process(
                self.handle,
                audio_data.as_ptr(),
                audio_data.len(),
                &mut probability,
                &mut flag,
            )
        };
        if ret != 0 {
            return Err(VadError::ProcessFailed);
        }
        Ok(VadResult {
            probability,
            is_voice: flag != 0,
        })
    }

    /// Return the configured hop size.
    pub fn hop_size(&self) -> HopSize {
        self.hop_size
    }

    /// Return the TEN VAD library version string (e.g. `"1.0.0"`).
    pub fn version() -> &'static str {
        let c_str = unsafe { ffi::ten_vad_get_version() };
        if c_str.is_null() {
            return "unknown";
        }
        unsafe { std::ffi::CStr::from_ptr(c_str) }
            .to_str()
            .unwrap_or("unknown")
    }
}

impl Drop for TenVad {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { ffi::ten_vad_destroy(&mut self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

// Safety: The native handle owns all its state internally and does not
// reference thread-local storage. Moving it between threads is safe.
unsafe impl Send for TenVad {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_threshold_negative() {
        let err = TenVad::new(HopSize::Samples256, -0.1).unwrap_err();
        assert!(matches!(err, VadError::InvalidThreshold(_)));
    }

    #[test]
    fn invalid_threshold_over_one() {
        let err = TenVad::new(HopSize::Samples256, 1.1).unwrap_err();
        assert!(matches!(err, VadError::InvalidThreshold(_)));
    }

    #[test]
    fn wrong_frame_size() {
        // This test requires the native library; skip if not available.
        let vad = TenVad::new(HopSize::Samples256, 0.5);
        if let Ok(mut vad) = vad {
            let too_short = vec![0i16; 100];
            let err = vad.process(&too_short).unwrap_err();
            match err {
                VadError::InvalidFrameSize { expected: 256, actual: 100 } => {}
                other => panic!("unexpected error: {other}"),
            }
        }
    }

    #[test]
    fn process_silence() {
        let vad = TenVad::new(HopSize::Samples256, 0.5);
        if let Ok(mut vad) = vad {
            let silence = vec![0i16; 256];
            let result = vad.process(&silence).unwrap();
            assert!(result.probability >= 0.0 && result.probability <= 1.0);
            // Silence should generally not be detected as voice.
            assert!(!result.is_voice, "silence detected as voice");
        }
    }

    #[test]
    fn version_is_non_empty() {
        let v = TenVad::version();
        // If the native library isn't linked, this may return "unknown".
        assert!(!v.is_empty());
    }

    #[test]
    fn hop_size_values() {
        assert_eq!(HopSize::Samples160.as_usize(), 160);
        assert_eq!(HopSize::Samples256.as_usize(), 256);
    }
}
```

## 8. Cargo.toml

```toml
[package]
name = "ten-vad"
version = "0.1.0"
edition = "2021"
description = "Rust bindings for TEN VAD (Voice Activity Detector)"
license = "Apache-2.0"
build = "build.rs"

[dependencies]
# No runtime dependencies — pure FFI wrapper

[dev-dependencies]
hound = "3.5"    # WAV file reading for examples/tests
```

## 9. 実装フェーズ

### Phase 1: クレート scaffold + FFI + build.rs
1. `crates/ten-vad/` ディレクトリ作成、ワークスペースに追加
2. `vendor/` に git submodule として TEN-framework/ten-vad を追加
3. `src/ffi.rs` — §6 の手書き FFI (4 関数)
4. `build.rs` — §5 の全処理集約版 (xcframework 変換 + リンク)
5. `cargo check -p ten-vad` が通ることを確認

### Phase 2: 安全な Rust API
1. `src/lib.rs` — §7 の完全実装
2. `HopSize` enum, `TenVad` struct, `VadResult`, `VadError`
3. テスト (threshold 検証, frame size 検証, silence 検出)

### Phase 3: Example + 動作確認
1. `examples/detect_vad.rs` — WAV → フレーム分割 → VAD 検出 → 結果出力
2. macOS で実動作確認

### Phase 4: 検証
1. `cargo test -p ten-vad`
2. `cargo clippy -p ten-vad -- -D warnings`
3. `cargo doc -p ten-vad --no-deps`
4. `cargo build -p ten-vad --release`
