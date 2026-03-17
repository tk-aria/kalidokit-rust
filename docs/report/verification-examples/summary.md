# 動作確認: examples — 作業報告

## 実行日時
2026-03-17 19:20-19:30 JST

## 作成した examples

### 1. decode_to_png_apple (VideoToolbox バックエンド)
```
cargo run -p video-decoder --example decode_to_png_apple -- <input.mp4> [output_dir]
```
- macOS VideoToolbox (AVFoundation) でHWデコード
- BGRA→RGBA swizzle して CPU バッファに格納
- PNG 出力

### 2. wgpu_video_bg (wgpu ウィンドウ動画背景)
```
cargo run -p video-decoder --example wgpu_video_bg -- <input.mp4>
```
- winit ウィンドウ + wgpu レンダリング
- SW デコーダ → queue.write_texture() → fullscreen quad 描画
- ESC で終了

## 動作確認結果

### decode_to_png (SW デコーダ)
- ✅ 10 フレーム正常出力 (640x360 RGBA PNG)
- ✅ Big Buck Bunny の正しい映像

### decode_to_png_apple (VideoToolbox)
- ✅ backend: VideoToolbox が選択される
- ✅ 10 フレーム正常出力 (640x360 RGBA PNG)
- ✅ Big Buck Bunny の正しい映像 (SW と同等品質)
- ✅ HW デコード確認 (AVFoundation → VideoToolbox)

### wgpu_video_bg (wgpu ウィンドウ)
- ✅ ウィンドウが起動
- ✅ 動画フレームが表示される
- ✅ 5秒間正常動作後に終了

## 実行コマンド
```
cargo run -p video-decoder --example decode_to_png -- big_buck_bunny_360p.mp4 /tmp/vd_frames
cargo run -p video-decoder --example decode_to_png_apple -- big_buck_bunny_360p.mp4 /tmp/vd_frames_apple
cargo run -p video-decoder --example wgpu_video_bg -- big_buck_bunny_360p.mp4
cargo test -p video-decoder   # 65 tests pass
cargo clippy -p video-decoder -- -D warnings  # clean
```
