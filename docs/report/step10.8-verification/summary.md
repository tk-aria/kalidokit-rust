# Phase 10: アプリ統合動作確認 — 作業報告

## 実行日時
2026-03-17 21:35-21:47 JST

## 手順

1. Big Buck Bunny MP4 を `assets/backgrounds/` にコピー
2. `user_prefs.yml` に `image_path: assets/backgrounds/big_buck_bunny_360p.mp4` を設定
3. `cargo run -p kalidokit-rust` で 8 秒間実行

## 確認結果

### 成功項目
- ✅ prefs から `image_path: Some("assets/backgrounds/big_buck_bunny_360p.mp4")` が正常読み込み
- ✅ `is_video_file()` で .mp4 拡張子を検出
- ✅ `Video background opened: assets/backgrounds/big_buck_bunny_360p.mp4 (640x360, 30.0fps, VideoToolbox)`
- ✅ VideoToolbox (AVFoundation HW decode) が使用された
- ✅ アプリが 8 秒間正常動作 (VRM モデル + 動画背景 + カメラ + トラッカー)
- ✅ save_prefs で image_path が保存される

### ログ出力 (抜粋)
```
[INFO] User prefs loaded: ... image_path: Some("assets/backgrounds/big_buck_bunny_360p.mp4") ...
[INFO] Video background opened: assets/backgrounds/big_buck_bunny_360p.mp4 (640x360, 30.0fps, VideoToolbox)
[INFO] Webcam initialized: 640x480 YUYV 30fps
[INFO] Idle animation loaded: 6.02s, 52 channels
[INFO] VCam TCP server listening on 127.0.0.1:19876
```

## 実行コマンド
```
cp crates/video-decoder/tests/fixtures/big_buck_bunny_360p.mp4 assets/backgrounds/
# user_prefs.yml を設定
RUST_LOG=info timeout 8 cargo run -p kalidokit-rust
```
