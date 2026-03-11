# Step 9.5: カメラ初期化の復元

## 作業日時
2026-03-11 14:27 JST

## 対象ファイル
- `crates/app/src/init.rs`

## 実行した操作

1. `use` 文を追加:
   - `nokhwa::pixel_format::RgbFormat`
   - `nokhwa::utils::{CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType}`
2. `init_camera()` 関数を実装:
   - `CameraIndex::Index(0)` でデフォルトカメラを選択
   - `RequestedFormat` で 640x480 MJPEG 30fps を要求
   - `Camera::new()` でカメラオブジェクト作成
   - `camera.open_stream()` でストリーム開始
   - 失敗時は `log::warn!` でメッセージを出し `None` を返す
3. `init_all()` 内のスタブを `init_camera()` 呼び出しに置換

## 実行コマンド
```bash
cargo check --workspace  # コンパイル成功確認
```

## 結果
- カメラ初期化が実装され、成功時は Some(Camera)、失敗時は None を返す
