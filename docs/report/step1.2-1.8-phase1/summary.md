# Phase 1: クレート基盤 + 共通型定義 — 作業報告

## 実行日時
2026-03-17 12:10-12:18 JST

## 完了タスク

### Step 1.1: Cargo.toml + ワークスペース追加
- ルート Cargo.toml に `"crates/video-decoder"` 追加
- `crates/video-decoder/Cargo.toml` 作成 (wgpu workspace 継承)
- `cargo check -p video-decoder` 成功

### Step 1.2: モジュール構造の scaffold
- lib.rs, error.rs, types.rs, handle.rs, session.rs 作成
- demux/mod.rs, nal/mod.rs, convert/mod.rs, backend/mod.rs, util/mod.rs 作成
- `cargo check -p video-decoder` 成功

### Step 1.3: エラー型 (error.rs)
- VideoError enum (9 variants) + Result type alias
- テスト: display_unsupported_codec, display_no_hw_decoder, display_format_mismatch, from_anyhow_error

### Step 1.4: 基本型 (types.rs)
- Codec, PixelFormat, ColorSpace, FrameStatus, Backend, VideoInfo
- テスト: color_space_default_is_bt709, codec_clone_and_eq, backend_debug_format, frame_status_variants, video_info_clone

### Step 1.5: NativeHandle (handle.rs)
- NativeHandle enum (Metal, D3d12, D3d11, Vulkan, Wgpu)
- unsafe impl Send + Sync
- テスト: native_handle_is_send_sync, wgpu_handle_create_and_clone, d3d12_handle_create

### Step 1.6: セッション定義 (session.rs)
- OutputTarget, SessionConfig, VideoSession trait
- テスト: session_config_default, output_target_construction

### Step 1.7: lib.rs re-exports + open()
- pub mod 宣言, pub use re-exports
- open() → backend::create_session()
- テスト: open_nonexistent_file_returns_file_not_found, open_stub_returns_no_hw_decoder

### Step 1.8: Phase 1 検証
- `cargo test -p video-decoder` — 16 tests + 1 doctest all pass
- `cargo clippy -p video-decoder -- -D warnings` — 0 warnings
- `cargo fmt -p video-decoder --check` — OK
- `cargo doc -p video-decoder --no-deps` — OK
- `cargo build -p video-decoder` — 成功
- `cargo-llvm-cov` 未インストールのためカバレッジ計測は保留

## 実行コマンド一覧
```
mkdir -p crates/video-decoder/src
# ファイル作成 (Write tool)
cargo check -p video-decoder  # expose-ids エラー → workspace 継承に修正
cargo check -p video-decoder  # 成功
# テストコード追加 (Edit tool)
cargo test -p video-decoder   # dyn VideoSession Debug エラー → match に修正
cargo test -p video-decoder   # 16 tests pass
cargo clippy -p video-decoder -- -D warnings  # derivable_impls → #[derive(Default)] + #[default] に修正
cargo fmt -p video-decoder
cargo clippy -p video-decoder -- -D warnings  # 0 warnings
cargo doc -p video-decoder --no-deps  # OK
```

## トラブルシューティング
1. wgpu `expose-ids` feature 不在 → `{ workspace = true }` で解決
2. `dyn VideoSession` に Debug 未実装 → `unwrap_err()` を `match` に変更
3. clippy `derivable_impls` → `#[derive(Default)]` + `#[default]` に変更
