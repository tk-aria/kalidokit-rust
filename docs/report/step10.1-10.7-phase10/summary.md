# Phase 10: kalidokit-rust アプリ統合 (Step 10.1-10.7) — 作業報告

## 実行日時
2026-03-17 21:20-21:28 JST

## 完了タスク

### Step 10.1: app Cargo.toml に video-decoder 依存追加
- `video-decoder = { path = "../video-decoder" }` 追加

### Step 10.2: VideoSession trait に frame_rgba() 追加
- session.rs: `fn frame_rgba(&self) -> Option<&[u8]>` (default: None)
- software.rs: override → `Some(&self.frame_buffer)`
- apple.rs: override → `Some(&self.frame_buffer)`

### Step 10.3: Scene に BgVideo 追加
- scene.rs: BgVideo struct (texture, bind_group, pipeline, width, height)
- set_background_video(), remove_background_video(), update_video_frame()
- render_to_view_with_depth() で bg_video > bg_image 優先
- BgImage::create_gpu_resources を pub(crate) に変更

### Step 10.4: AppState に video_session 追加
- state.rs: `pub video_session: Option<Box<dyn video_decoder::VideoSession>>`

### Step 10.5: init.rs で拡張子判定
- is_video_file() ヘルパー (.mp4/.m4v/.mov)
- image_path が動画 → video-decoder open + set_background_video
- image_path が静止画/GIF → 既存パス

### Step 10.6: update.rs で毎フレームデコード
- decode_frame() → NewFrame → frame_rgba() → update_video_frame()
- 動画未使用時のみ tick_background()

### Step 10.7: app.rs に KeyP ショートカット
- KeyP: pause/resume トグル

## 検証結果
- `cargo check --workspace` — OK
- `cargo test -p video-decoder` — 75 tests pass
- `cargo test -p renderer -p vrm -p solver` — 88 tests pass
- `cargo clippy --workspace -- -D warnings` — OK
- `cargo fmt --check` — OK

## 実行コマンド
```
# Step 10.1
vi crates/app/Cargo.toml  # video-decoder 依存追加
cargo check -p kalidokit-rust

# Steps 10.2-10.7 (subagent)
cargo check --workspace
cargo test -p video-decoder
cargo test -p renderer -p vrm -p solver
cargo clippy --workspace -- -D warnings
cargo fmt --check
```
