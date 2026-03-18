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

## 3. プリビルドバイナリ

| プラットフォーム | ファイル | アーキテクチャ |
|---|---|---|
| Linux | `lib/Linux/x64/libten_vad.so` | x86_64 |
| Windows | `lib/Windows/x64/ten_vad.dll` + `ten_vad.lib` | x86_64 |
| Windows | `lib/Windows/x86/ten_vad.dll` + `ten_vad.lib` | x86 |
| macOS | `lib/macOS/ten_vad.framework/` | arm64 + x86_64 (universal) |
| Android | `lib/Android/arm64-v8a/libten_vad.so` | arm64 |
| Android | `lib/Android/armeabi-v7a/libten_vad.so` | armv7 |
| iOS | `lib/iOS/ten_vad.framework/` | arm64 |

## 4. クレート構成

```
crates/ten-vad/
├── Cargo.toml
├── build.rs              # プリビルトライブラリのリンク設定
├── src/
│   ├── lib.rs            # 安全な Rust API (TenVad struct)
│   └── ffi.rs            # bindgen 生成 or 手書き FFI 宣言
├── ten-vad-vendor/       # git submodule or ダウンロードスクリプト
│   ├── include/
│   │   └── ten_vad.h
│   └── lib/
│       ├── Linux/x64/libten_vad.so
│       ├── Windows/x64/ten_vad.dll
│       ├── macOS/ten_vad.framework/
│       └── ...
└── examples/
    └── detect_vad.rs     # WAV ファイルから VAD 検出
```

## 5. アプローチ: bindgen vs 手書き FFI

API が 4 関数と非常にシンプルなため **手書き FFI** を推奨:

```rust
// src/ffi.rs
use std::ffi::c_void;
use std::os::raw::{c_char, c_float, c_int};

pub type TenVadHandle = *mut c_void;

extern "C" {
    pub fn ten_vad_create(
        handle: *mut TenVadHandle,
        hop_size: usize,
        threshold: c_float,
    ) -> c_int;

    pub fn ten_vad_process(
        handle: TenVadHandle,
        audio_data: *const i16,
        audio_data_length: usize,
        out_probability: *mut c_float,
        out_flag: *mut c_int,
    ) -> c_int;

    pub fn ten_vad_destroy(handle: *mut TenVadHandle) -> c_int;

    pub fn ten_vad_get_version() -> *const c_char;
}
```

**理由**: bindgen は libclang 依存が発生し、CI/ビルド環境を複雑にする。4 関数なら手書きの方が軽量。

## 6. 安全な Rust API

```rust
// src/lib.rs
mod ffi;

/// Voice Activity Detection result.
pub struct VadResult {
    /// Probability of voice activity [0.0, 1.0].
    pub probability: f32,
    /// Binary flag: true if voice detected.
    pub is_voice: bool,
}

/// TEN VAD instance.
pub struct TenVad {
    handle: ffi::TenVadHandle,
    hop_size: usize,
}

impl TenVad {
    /// Create a new VAD instance.
    ///
    /// - `hop_size`: Frame size in samples (160 or 256 for 16kHz audio).
    /// - `threshold`: Detection threshold [0.0, 1.0].
    pub fn new(hop_size: usize, threshold: f32) -> Result<Self, VadError> {
        let mut handle = std::ptr::null_mut();
        let ret = unsafe { ffi::ten_vad_create(&mut handle, hop_size, threshold) };
        if ret != 0 || handle.is_null() {
            return Err(VadError::CreateFailed);
        }
        Ok(Self { handle, hop_size })
    }

    /// Process one audio frame.
    ///
    /// `audio_data` must have exactly `hop_size` samples (16kHz, 16-bit PCM).
    pub fn process(&mut self, audio_data: &[i16]) -> Result<VadResult, VadError> {
        if audio_data.len() != self.hop_size {
            return Err(VadError::InvalidFrameSize {
                expected: self.hop_size,
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

    /// Get the library version string.
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

// Safety: TEN VAD handle is thread-safe per the library documentation.
unsafe impl Send for TenVad {}
```

## 7. build.rs — プリビルトライブラリリンク

```rust
// build.rs
fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let lib_dir = match (target_os.as_str(), target_arch.as_str()) {
        ("linux", "x86_64") => format!("{}/ten-vad-vendor/lib/Linux/x64", manifest_dir),
        ("windows", "x86_64") => format!("{}/ten-vad-vendor/lib/Windows/x64", manifest_dir),
        ("windows", "x86") => format!("{}/ten-vad-vendor/lib/Windows/x86", manifest_dir),
        ("macos", _) => format!("{}/ten-vad-vendor/lib/macOS", manifest_dir),
        ("android", "aarch64") => format!("{}/ten-vad-vendor/lib/Android/arm64-v8a", manifest_dir),
        ("android", "arm") => format!("{}/ten-vad-vendor/lib/Android/armeabi-v7a", manifest_dir),
        ("ios", "aarch64") => format!("{}/ten-vad-vendor/lib/iOS", manifest_dir),
        _ => panic!("Unsupported target: {}-{}", target_os, target_arch),
    };

    match target_os.as_str() {
        "macos" | "ios" => {
            println!("cargo:rustc-link-search=framework={}", lib_dir);
            println!("cargo:rustc-link-lib=framework=ten_vad");
        }
        "windows" => {
            println!("cargo:rustc-link-search=native={}", lib_dir);
            println!("cargo:rustc-link-lib=dylib=ten_vad");
        }
        _ => {
            println!("cargo:rustc-link-search=native={}", lib_dir);
            println!("cargo:rustc-link-lib=dylib=ten_vad");
        }
    }
}
```

## 8. 実装フェーズ

### Phase 1: クレート scaffold + FFI
1. `crates/ten-vad/` ディレクトリ作成、ワークスペースに追加
2. `ten-vad-vendor/` にプリビルトバイナリ + ヘッダーを配置 (git submodule or download script)
3. `ffi.rs` — 手書き FFI 宣言 (4 関数)
4. `build.rs` — プラットフォーム別リンク設定
5. `cargo check -p ten-vad` が通ることを確認

### Phase 2: 安全な Rust API
1. `TenVad` struct (new, process, drop)
2. `VadResult` struct (probability, is_voice)
3. `VadError` enum (CreateFailed, ProcessFailed, InvalidFrameSize)
4. `TenVad::version()` 静的メソッド
5. `unsafe impl Send for TenVad`

### Phase 3: テスト
1. 正常系: create → process (無音フレーム) → destroy
2. 正常系: create → process (音声フレーム) → is_voice == true
3. 異常系: 不正な hop_size → エラー
4. 異常系: フレームサイズ不一致 → VadError::InvalidFrameSize
5. version() が非空文字列を返す

### Phase 4: Example
1. `examples/detect_vad.rs` — WAV ファイルを読み込んで VAD 検出結果を出力
2. `cargo run -p ten-vad --example detect_vad -- input.wav`

### Phase 5: 検証
1. `cargo test -p ten-vad`
2. `cargo clippy -p ten-vad -- -D warnings`
3. `cargo doc -p ten-vad --no-deps`
4. macOS で実動作確認 (16kHz WAV)
