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
| macOS | `lib/macOS/ten_vad.xcframework/` | arm64 + x86_64 (universal) |
| Android | `lib/Android/arm64-v8a/libten_vad.so` | arm64 |
| Android | `lib/Android/armeabi-v7a/libten_vad.so` | armv7 |
| iOS | `lib/iOS/ten_vad.xcframework/` | arm64 (device) + x86_64 (simulator) |

### .xcframework について

上流の TEN VAD は `.framework` 形式で提供しているが、本クレートでは `.xcframework` に変換して使用する。

**理由:**
- `.xcframework` は Apple 推奨の配布形式 (Xcode 11+)
- 複数アーキテクチャ・プラットフォーム (device + simulator) を1パッケージに格納可能
- iOS: arm64 (device) と x86_64 (simulator) を同一 xcframework で管理
- macOS: arm64 と x86_64 を同一 xcframework で管理
- lipo universal binary の「同一アーキテクチャ重複」問題を回避

**変換スクリプト (`scripts/build_xcframeworks.sh`):**
```bash
#!/bin/bash
# 上流 .framework → .xcframework 変換

# macOS: 元の framework がすでに universal (arm64 + x86_64) の場合そのまま
xcodebuild -create-xcframework \
  -framework lib/macOS/ten_vad.framework \
  -output lib/macOS/ten_vad.xcframework

# iOS: device (arm64) と simulator (x86_64) を分離して結合
# ※上流が arm64 のみ提供の場合、simulator 用は別途ビルドまたは
#   ソースからクロスコンパイルが必要
xcodebuild -create-xcframework \
  -framework lib/iOS/ten_vad.framework \
  -framework lib/iOS-simulator/ten_vad.framework \
  -output lib/iOS/ten_vad.xcframework
```

**iOS x86_64 (Simulator) バイナリの取得方法:**
1. 上流が提供していれば使用
2. 提供していなければ `src/` の C++ ソースから `x86_64-apple-ios-simulator` ターゲットでクロスコンパイル
3. いずれも不可の場合、iOS simulator 対応は feature flag でオプション化

## 4. クレート構成

```
crates/ten-vad/
├── Cargo.toml
├── build.rs                    # プリビルトライブラリのリンク設定
├── src/
│   ├── lib.rs                  # 安全な Rust API (TenVad struct)
│   └── ffi.rs                  # 手書き FFI 宣言 (4 関数)
├── ten-vad-vendor/             # git submodule or ダウンロードスクリプト
│   ├── include/
│   │   └── ten_vad.h
│   └── lib/
│       ├── Linux/x64/libten_vad.so
│       ├── Windows/x64/ten_vad.dll + ten_vad.lib
│       ├── Windows/x86/ten_vad.dll + ten_vad.lib
│       ├── macOS/ten_vad.xcframework/
│       │   ├── Info.plist
│       │   ├── macos-arm64_x86_64/ten_vad.framework/
│       │   └── (universal binary)
│       ├── iOS/ten_vad.xcframework/
│       │   ├── Info.plist
│       │   ├── ios-arm64/ten_vad.framework/
│       │   └── ios-arm64_x86_64-simulator/ten_vad.framework/
│       └── Android/
│           ├── arm64-v8a/libten_vad.so
│           └── armeabi-v7a/libten_vad.so
├── scripts/
│   └── build_xcframeworks.sh   # .framework → .xcframework 変換
└── examples/
    └── detect_vad.rs           # WAV ファイルから VAD 検出
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

## 7. build.rs — プリビルトライブラリリンク (.xcframework 対応)

```rust
// build.rs
fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let vendor = format!("{}/ten-vad-vendor/lib", manifest_dir);

    match (target_os.as_str(), target_arch.as_str()) {
        // --- Linux ---
        ("linux", "x86_64") => {
            println!("cargo:rustc-link-search=native={}/Linux/x64", vendor);
            println!("cargo:rustc-link-lib=dylib=ten_vad");
        }

        // --- Windows ---
        ("windows", arch) => {
            let dir = if arch == "x86_64" { "x64" } else { "x86" };
            println!("cargo:rustc-link-search=native={}/Windows/{}", vendor, dir);
            println!("cargo:rustc-link-lib=dylib=ten_vad");
        }

        // --- macOS (.xcframework) ---
        ("macos", _) => {
            // xcframework 内の macOS 用 framework を探す
            // 構造: ten_vad.xcframework/macos-arm64_x86_64/ten_vad.framework/
            let fw_dir = format!("{}/macOS/ten_vad.xcframework/macos-arm64_x86_64", vendor);
            println!("cargo:rustc-link-search=framework={}", fw_dir);
            println!("cargo:rustc-link-lib=framework=ten_vad");
        }

        // --- iOS (.xcframework) ---
        ("ios", _) => {
            // iOS simulator は target_env="sim" or target triple に "sim" が含まれる
            let is_simulator = target_env.contains("sim")
                || std::env::var("TARGET").unwrap_or_default().contains("sim");
            let slice = if is_simulator {
                "ios-arm64_x86_64-simulator"
            } else {
                "ios-arm64"
            };
            let fw_dir = format!("{}/iOS/ten_vad.xcframework/{}", vendor, slice);
            println!("cargo:rustc-link-search=framework={}", fw_dir);
            println!("cargo:rustc-link-lib=framework=ten_vad");
        }

        // --- Android ---
        ("android", "aarch64") => {
            println!("cargo:rustc-link-search=native={}/Android/arm64-v8a", vendor);
            println!("cargo:rustc-link-lib=dylib=ten_vad");
        }
        ("android", "arm") => {
            println!("cargo:rustc-link-search=native={}/Android/armeabi-v7a", vendor);
            println!("cargo:rustc-link-lib=dylib=ten_vad");
        }

        _ => panic!("Unsupported target: {}-{}", target_os, target_arch),
    }
}
```

**xcframework リンクのポイント:**
- `.xcframework` は直接リンクできない。内部の platform slice (e.g. `macos-arm64_x86_64/ten_vad.framework/`) を `framework` search path に指定する
- iOS: device (`ios-arm64`) と simulator (`ios-arm64_x86_64-simulator`) を `TARGET` 環境変数で判定
- macOS: universal binary は1つの slice に arm64 + x86_64 両方が入っている

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
