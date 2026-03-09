# Webcam Capture Implementation - 作業報告

## タスク
- features.md line 1067: `Webカメラ初期化 (nokhwa) — 未実装`
- features.md line 1074: `[ ] update_frame: Webカメラからフレーム取得`

## 実施内容

### 1. AppState にカメラフィールド追加
- `crates/app/src/state.rs`: `camera: Option<nokhwa::Camera>` フィールド追加

### 2. init.rs でWebカメラ初期化
- `init_camera()` 関数追加
  - CameraIndex::Index(0) でデフォルトカメラ
  - 640x480 MJPEG 30fps リクエスト
  - open_stream() でストリーム開始
  - 失敗時は log::warn で警告出力、None を返す（クラッシュしない）

### 3. update.rs でフレーム取得
- `capture_frame()` 関数追加
  - camera.frame() でフレーム取得
  - decode_image::<RgbFormat>() で DynamicImage に変換
  - 実際のカメラ解像度で VideoInfo を更新
  - カメラ未接続またはフレーム取得失敗時はダミー黒画像にフォールバック

## 実行コマンド
- `cargo check --workspace` — コンパイル成功

## 結果
`[x]` に更新。Webカメラキャプチャ実装完了（グレースフルフォールバック付き）。
