# Step 9.6: フレーム取得の復元

## 作業日時
2026-03-11 14:27 JST

## 対象ファイル
- `crates/app/src/update.rs`

## 実行した操作

1. `use nokhwa::pixel_format::RgbFormat;` を追加
2. `capture_frame()` の引数型を `&mut Option<()>` → `&mut Option<nokhwa::Camera>` に変更
3. カメラが `Some` の場合の実装:
   - `camera.frame()` でバッファ取得
   - `buffer.resolution()` で実際の解像度を取得
   - `buffer.decode_image::<RgbFormat>()` でデコード
   - `DynamicImage::ImageRgb8(rgb_image)` でラップ
4. カメラが `None` またはフレーム取得失敗時: 640x480 ダミー黒画像にフォールバック
5. 解像度を VideoInfo に動的反映（ハードコードしない）

## 実行コマンド
```bash
cargo check --workspace  # コンパイル成功確認
```

## 結果
- 実カメラからのフレーム取得が復元された
- フォールバック機構も維持
