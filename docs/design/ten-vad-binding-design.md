# ten-vad-rs — Rust バインディング設計

## 1. 概要

[TEN VAD](https://github.com/TEN-framework/ten-vad) の Rust バインディングクレート。
**上流が提供するプリビルトバイナリ** をリンクして使用する。ソースからのビルドは行わない。

## 2. TEN VAD C API (include/ten_vad.h)

```c
typedef void *ten_vad_handle_t;

int ten_vad_create(ten_vad_handle_t *handle, size_t hop_size, float threshold);
int ten_vad_process(ten_vad_handle_t handle, const int16_t *audio_data,
                    size_t audio_data_length, float *out_probability, int *out_flag);
int ten_vad_destroy(ten_vad_handle_t *handle);
const char *ten_vad_get_version(void);
```

- 入力: 16kHz 16bit PCM, フレームサイズ 160 or 256 サンプル
- 出力: 確率 [0.0, 1.0] + バイナリフラグ (0/1)

## 3. ビルド方針

**プリビルトバイナリのみ使用。C/C++ ソースのコンパイルは行わない。**

- build.rs はリンク設定と Apple プラットフォームでの .framework → .xcframework 変換のみ
- 上流の `lib/` ディレクトリにあるバイナリをそのままリンク
- ソースビルドが必要な場合は将来 `build-from-source` feature flag で対応

| プラットフォーム | 上流提供物 | build.rs の処理 |
|---|---|---|
| Linux | `libten_vad.so` | link-search + link-lib 出力のみ |
| Windows | `ten_vad.dll` + `.lib` | link-search + link-lib 出力のみ |
| macOS | `ten_vad.framework` | xcodebuild で .xcframework に変換 → link |
| iOS | `ten_vad.framework` | xcodebuild で .xcframework に変換 → link |
| Android | `libten_vad.so` | link-search + link-lib 出力のみ |

## 4. クレート構成

```
crates/ten-vad/
├── Cargo.toml
├── build.rs
├── src/
│   ├── lib.rs
│   └── ffi.rs
├── vendor/                     # git submodule: TEN-framework/ten-vad
│   ├── include/ten_vad.h
│   └── lib/
│       ├── Linux/x64/libten_vad.so
│       ├── Windows/x64/ten_vad.dll + ten_vad.lib
│       ├── Windows/x86/ten_vad.dll + ten_vad.lib
│       ├── macOS/ten_vad.framework/
│       ├── iOS/ten_vad.framework/
│       └── Android/
│           ├── arm64-v8a/libten_vad.so
│           └── armeabi-v7a/libten_vad.so
├── xcframework/                # build.rs が生成 (.gitignore)
└── examples/
    └── detect_vad.rs
```

## 5. Cargo.toml

```toml
[package]
name = "ten-vad"
version = "0.1.0"
edition = "2021"
description = "Rust bindings for TEN VAD (Voice Activity Detector)"
license = "Apache-2.0"
build = "build.rs"

[dependencies]
# ランタイム依存なし — pure FFI ラッパー

[dev-dependencies]
hound = "3.5"    # WAV 読み込み (example/test 用)
```

## 6. build.rs — 全文

```rust
//! build.rs — プリビルトバイナリのリンク設定
//!
//! - Linux/Windows/Android: link-search + link-lib を出力するだけ
//! - macOS/iOS: 上流の .framework を xcodebuild で .xcframework に変換してからリンク
//! - C/C++ ソースのコンパイルは一切行わない

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

// ── Linux ────────────────────────────────────────────────────
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

// ── Windows ──────────────────────────────────────────────────
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

// ── macOS ────────────────────────────────────────────────────
fn link_macos(vendor_lib: &Path, cache: &Path) {
    let xcfw = cache.join("macOS/ten_vad.xcframework");

    // キャッシュに xcframework が無ければ、上流 .framework から変換
    if !xcfw.join("Info.plist").exists() {
        let src = vendor_lib.join("macOS/ten_vad.framework");
        assert!(src.exists(), "Missing: {}", src.display());
        create_xcframework(&[&src], &xcfw);
    }

    // xcframework 内の最初の slice を探してリンク
    let slice = find_slice(&xcfw, "macos");
    println!("cargo:rustc-link-search=framework={}", slice.display());
    println!("cargo:rustc-link-lib=framework=ten_vad");
}

// ── iOS ──────────────────────────────────────────────────────
fn link_ios(vendor_lib: &Path, cache: &Path, arch: &str) {
    let xcfw = cache.join("iOS/ten_vad.xcframework");

    if !xcfw.join("Info.plist").exists() {
        let device_fw = vendor_lib.join("iOS/ten_vad.framework");
        assert!(device_fw.exists(), "Missing: {}", device_fw.display());

        // simulator framework があれば device + sim を束ねる
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

    let is_sim = env::var("TARGET")
        .unwrap_or_default()
        .contains("sim")
        || arch == "x86_64";
    let keyword = if is_sim { "simulator" } else { "ios-arm64" };
    let slice = find_slice(&xcfw, keyword);
    println!("cargo:rustc-link-search=framework={}", slice.display());
    println!("cargo:rustc-link-lib=framework=ten_vad");
}

// ── Android ──────────────────────────────────────────────────
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

// ── ヘルパー ─────────────────────────────────────────────────

/// xcodebuild -create-xcframework を実行する。
/// `frameworks` は入力 .framework パスの配列。
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
        "xcodebuild -create-xcframework failed (exit {})",
        status
    );
    eprintln!("cargo:warning=Created xcframework: {}", output.display());
}

/// xcframework 内のディレクトリから `keyword` を含む slice を探す。
fn find_slice(xcfw: &Path, keyword: &str) -> PathBuf {
    for entry in std::fs::read_dir(xcfw)
        .unwrap_or_else(|_| panic!("Cannot read {}", xcfw.display()))
        .flatten()
    {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name();
        if name.to_string_lossy().contains(keyword) {
            return entry.path();
        }
    }
    panic!(
        "No slice matching '{keyword}' in {}",
        xcfw.display()
    );
}
```

## 7. src/ffi.rs — 全文

```rust
//! Raw FFI bindings to the TEN VAD C library.
//!
//! These declarations correspond exactly to `include/ten_vad.h`.
//! The library is linked at compile time by `build.rs`.

use std::ffi::c_void;
use std::os::raw::{c_char, c_float, c_int};

/// Opaque handle for a TEN VAD instance.
pub type TenVadHandle = *mut c_void;

extern "C" {
    /// Create and initialize a TEN VAD instance.
    ///
    /// # Parameters
    /// - `handle`: Receives the allocated handle on success.
    /// - `hop_size`: Samples per frame (160 or 256 for 16 kHz).
    /// - `threshold`: Detection threshold in [0.0, 1.0].
    ///
    /// # Returns
    /// 0 on success, -1 on failure.
    pub fn ten_vad_create(
        handle: *mut TenVadHandle,
        hop_size: usize,
        threshold: c_float,
    ) -> c_int;

    /// Process one audio frame.
    ///
    /// # Parameters
    /// - `handle`: Valid handle from `ten_vad_create`.
    /// - `audio_data`: `hop_size` samples of 16-bit PCM at 16 kHz.
    /// - `audio_data_length`: Must equal `hop_size`.
    /// - `out_probability`: Receives voice probability [0.0, 1.0].
    /// - `out_flag`: Receives 1 (voice) or 0 (no voice).
    ///
    /// # Returns
    /// 0 on success, -1 on failure.
    pub fn ten_vad_process(
        handle: TenVadHandle,
        audio_data: *const i16,
        audio_data_length: usize,
        out_probability: *mut c_float,
        out_flag: *mut c_int,
    ) -> c_int;

    /// Destroy a TEN VAD instance.
    ///
    /// Sets `*handle` to NULL on success.
    ///
    /// # Returns
    /// 0 on success, -1 on failure.
    pub fn ten_vad_destroy(handle: *mut TenVadHandle) -> c_int;

    /// Return the library version string (e.g. "1.0.0").
    ///
    /// The pointer is valid for the lifetime of the process.
    pub fn ten_vad_get_version() -> *const c_char;
}
```

## 8. src/lib.rs — 全文

```rust
//! # ten-vad
//!
//! Rust bindings for [TEN VAD](https://github.com/TEN-framework/ten-vad),
//! a low-latency, high-performance voice activity detector.
//!
//! Uses prebuilt native libraries — no C/C++ compilation required.
//!
//! ## Example
//!
//! ```rust,no_run
//! use ten_vad::{TenVad, HopSize};
//!
//! let mut vad = TenVad::new(HopSize::Samples256, 0.5).unwrap();
//! println!("TEN VAD {}", TenVad::version());
//!
//! let silence = vec![0i16; 256];
//! let result = vad.process(&silence).unwrap();
//! println!("voice={} prob={:.3}", result.is_voice, result.probability);
//! ```

pub mod ffi;

use std::fmt;

// ── 型定義 ───────────────────────────────────────────────────

/// Supported hop (frame) sizes for 16 kHz audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Voice activity detection result for one audio frame.
#[derive(Debug, Clone, Copy)]
pub struct VadResult {
    /// Probability of voice presence in [0.0, 1.0].
    pub probability: f32,
    /// `true` when probability ≥ threshold.
    pub is_voice: bool,
}

/// Errors from VAD operations.
#[derive(Debug)]
pub enum VadError {
    /// `ten_vad_create` failed.
    CreateFailed,
    /// `ten_vad_process` failed.
    ProcessFailed,
    /// Audio frame length ≠ configured hop size.
    InvalidFrameSize { expected: usize, actual: usize },
    /// Threshold outside [0.0, 1.0].
    InvalidThreshold(f32),
}

impl fmt::Display for VadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateFailed => write!(f, "ten_vad_create failed"),
            Self::ProcessFailed => write!(f, "ten_vad_process failed"),
            Self::InvalidFrameSize { expected, actual } => {
                write!(f, "frame size: expected {expected}, got {actual}")
            }
            Self::InvalidThreshold(t) => {
                write!(f, "threshold {t} outside [0.0, 1.0]")
            }
        }
    }
}

impl std::error::Error for VadError {}

// ── TenVad ───────────────────────────────────────────────────

/// A TEN VAD voice activity detector.
///
/// Wraps the native C handle. Destroyed automatically on [`Drop`].
///
/// # Thread Safety
/// `Send` but not `Sync` — move between threads but do not share.
pub struct TenVad {
    handle: ffi::TenVadHandle,
    hop_size: HopSize,
}

impl TenVad {
    /// Create a new detector.
    ///
    /// - `hop_size`: `Samples160` (10 ms) or `Samples256` (16 ms) at 16 kHz.
    /// - `threshold`: Sensitivity in [0.0, 1.0]. Lower = more aggressive.
    pub fn new(hop_size: HopSize, threshold: f32) -> Result<Self, VadError> {
        if !(0.0..=1.0).contains(&threshold) {
            return Err(VadError::InvalidThreshold(threshold));
        }
        let mut handle: ffi::TenVadHandle = std::ptr::null_mut();
        let ret = unsafe { ffi::ten_vad_create(&mut handle, hop_size.as_usize(), threshold) };
        if ret != 0 || handle.is_null() {
            return Err(VadError::CreateFailed);
        }
        Ok(Self { handle, hop_size })
    }

    /// Process one frame of 16 kHz 16-bit PCM audio.
    ///
    /// `audio` must contain exactly [`hop_size`](Self::hop_size) samples.
    pub fn process(&mut self, audio: &[i16]) -> Result<VadResult, VadError> {
        let expected = self.hop_size.as_usize();
        if audio.len() != expected {
            return Err(VadError::InvalidFrameSize {
                expected,
                actual: audio.len(),
            });
        }
        let mut prob: f32 = 0.0;
        let mut flag: i32 = 0;
        let ret = unsafe {
            ffi::ten_vad_process(self.handle, audio.as_ptr(), audio.len(), &mut prob, &mut flag)
        };
        if ret != 0 {
            return Err(VadError::ProcessFailed);
        }
        Ok(VadResult {
            probability: prob,
            is_voice: flag != 0,
        })
    }

    /// Configured hop size.
    pub fn hop_size(&self) -> HopSize {
        self.hop_size
    }

    /// Library version (e.g. `"1.0.0"`).
    pub fn version() -> &'static str {
        let ptr = unsafe { ffi::ten_vad_get_version() };
        if ptr.is_null() {
            return "unknown";
        }
        unsafe { std::ffi::CStr::from_ptr(ptr) }
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

unsafe impl Send for TenVad {}

// ── テスト ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_below_zero() {
        assert!(matches!(
            TenVad::new(HopSize::Samples256, -0.1),
            Err(VadError::InvalidThreshold(_))
        ));
    }

    #[test]
    fn threshold_above_one() {
        assert!(matches!(
            TenVad::new(HopSize::Samples256, 1.1),
            Err(VadError::InvalidThreshold(_))
        ));
    }

    #[test]
    fn wrong_frame_length() {
        if let Ok(mut vad) = TenVad::new(HopSize::Samples256, 0.5) {
            let short = vec![0i16; 100];
            match vad.process(&short) {
                Err(VadError::InvalidFrameSize {
                    expected: 256,
                    actual: 100,
                }) => {}
                other => panic!("expected InvalidFrameSize, got {other:?}"),
            }
        }
    }

    #[test]
    fn silence_is_not_voice() {
        if let Ok(mut vad) = TenVad::new(HopSize::Samples256, 0.5) {
            let silence = vec![0i16; 256];
            let r = vad.process(&silence).unwrap();
            assert!((0.0..=1.0).contains(&r.probability));
            assert!(!r.is_voice, "silence detected as voice");
        }
    }

    #[test]
    fn version_non_empty() {
        let v = TenVad::version();
        assert!(!v.is_empty());
    }

    #[test]
    fn hop_size_values() {
        assert_eq!(HopSize::Samples160.as_usize(), 160);
        assert_eq!(HopSize::Samples256.as_usize(), 256);
    }
}
```

## 9. examples/detect_vad.rs — 全文

```rust
//! Read a 16 kHz mono WAV file, run VAD frame-by-frame, and print results.
//!
//! ```sh
//! cargo run -p ten-vad --example detect_vad -- input.wav
//! ```

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            eprintln!("Usage: detect_vad <input.wav>");
            std::process::exit(1);
        });

    let mut reader = hound::WavReader::open(&path)?;
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16000, "Expected 16 kHz, got {}", spec.sample_rate);
    assert_eq!(spec.channels, 1, "Expected mono, got {} channels", spec.channels);

    let hop = ten_vad::HopSize::Samples256;
    let mut vad = ten_vad::TenVad::new(hop, 0.5)?;
    println!("TEN VAD {}", ten_vad::TenVad::version());
    println!("File: {path}  ({} Hz, {} ch)", spec.sample_rate, spec.channels);

    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    let frame_size = hop.as_usize();
    let mut voice_frames = 0u32;
    let mut total_frames = 0u32;

    for (i, chunk) in samples.chunks_exact(frame_size).enumerate() {
        let r = vad.process(chunk)?;
        total_frames += 1;
        if r.is_voice {
            voice_frames += 1;
        }
        let time_ms = i * frame_size * 1000 / 16000;
        if r.is_voice {
            println!("[{time_ms:>6} ms] VOICE  prob={:.3}", r.probability);
        }
    }

    println!(
        "\nSummary: {voice_frames}/{total_frames} frames contain voice ({:.1}%)",
        voice_frames as f64 / total_frames.max(1) as f64 * 100.0
    );
    Ok(())
}
```

## 10. 実装フェーズ

### Phase 1: クレート scaffold
1. `crates/ten-vad/` 作成、ワークスペースに追加
2. `vendor/` に git submodule 追加: `git submodule add https://github.com/TEN-framework/ten-vad vendor`
3. `Cargo.toml`, `build.rs`, `src/ffi.rs`, `src/lib.rs` を §5-8 の通り配置
4. `cargo check -p ten-vad` 通過確認

### Phase 2: 動作確認
1. `cargo test -p ten-vad` (ネイティブライブラリがリンクできれば全テスト pass)
2. `cargo run -p ten-vad --example detect_vad -- test.wav`
3. `cargo clippy -p ten-vad -- -D warnings`
4. `cargo doc -p ten-vad --no-deps`
