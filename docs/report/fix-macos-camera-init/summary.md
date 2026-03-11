# Fix: macOS カメラ初期化の修正

## 作業日時
2026-03-11 15:57 JST

## 問題
MacBook Air の内蔵カメラが `Failed to create camera` で取得できない。

## 原因
nokhwa 0.10 の macOS (AVFoundation) バックエンドでは、カメラ使用前に以下が必要:

1. **`nokhwa_initialize()` の呼び出し**: AVFoundation の `requestAccessForMediaType:completionHandler:` でカメラ許可をリクエスト
2. **YUYV フォーマットの使用**: macOS 内蔵カメラは通常 MJPEG を直接サポートせず、NV12/YUYV を使用

## 修正内容

### `crates/app/src/init.rs` — `init_camera()`

1. `nokhwa::nokhwa_initialize()` をコールバック付きで呼び出し、カメラ許可を取得
   - `std::sync::mpsc::channel` で許可結果を同期的に受信
   - 30秒タイムアウト付き
   - 許可が拒否された場合はエラーを返す
2. `FrameFormat::MJPEG` → `FrameFormat::YUYV` に変更
   - macOS 内蔵カメラは YUYV が標準
   - `RequestedFormatType::Closest` なので、利用可能な最も近いフォーマットにフォールバック

## 実行コマンド
```bash
cargo check --workspace  # コンパイル成功
cargo fmt                # フォーマット修正
cargo clippy --workspace -- -D warnings  # lint パス
```
