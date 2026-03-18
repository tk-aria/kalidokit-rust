# Phase 13: ten-vad Rust バインディング — 作業報告

## 実行日時
2026-03-19 00:40-00:50 JST

## 完了タスク

### Step 13.1: クレート scaffold
- crates/ten-vad/ 作成、ワークスペースに追加
- git submodule add TEN-framework/ten-vad → vendor/
- Cargo.toml, .gitignore

### Step 13.2: build.rs
- プリビルトバイナリリンク (Linux/Windows/Android: link-search + link-lib)
- macOS: xcodebuild -create-xcframework で .framework → .xcframework 変換
- @rpath 設定で dyld がフレームワークを発見可能に

### Step 13.3: 手書き FFI (ffi.rs)
- ten_vad_create, ten_vad_process, ten_vad_destroy, ten_vad_get_version

### Step 13.4: 安全な Rust API (lib.rs)
- TenVad struct, HopSize enum, VadResult, VadError
- Drop 実装, Send 実装

### Step 13.5: Example (detect_vad.rs)
- WAV → フレーム分割 → VAD → 結果表示

### Step 13.6: テスト
- 6 tests + 1 doctest pass
- clippy clean, fmt clean, doc clean

## トラブルシューティング
- dyld "Library not loaded: @rpath/ten_vad.framework" → build.rs に cargo:rustc-link-arg=-Wl,-rpath 追加で解決

## 実行コマンド
```
git submodule add https://github.com/TEN-framework/ten-vad crates/ten-vad/vendor
cargo check -p ten-vad
cargo test -p ten-vad    # 6 tests + 1 doctest pass
cargo clippy -p ten-vad -- -D warnings
cargo fmt -p ten-vad
cargo doc -p ten-vad --no-deps
```
