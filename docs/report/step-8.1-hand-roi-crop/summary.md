# Step 8.1: Pose → Hand ROI クロップ

## 完了日時
2026-03-10 12:54 JST

## 変更ファイル
- `crates/tracker/src/preprocess.rs` — `calc_hand_roi()`, `crop_image()` 追加 + 4テスト追加
- `crates/tracker/src/holistic.rs` — `detect()` を修正: Pose wrist→ROI クロップ→Hand 推論

## 実行コマンド
```bash
cargo check --workspace        # OK
cargo test -p solver -p vrm -p renderer  # 28 tests passed (tracker は ort-sys 制約で除外)
```

## 実装内容
1. `calc_hand_roi(wrist: Vec2, image_width, image_height) -> (u32, u32, u32, u32)` 追加
   - 正規化された手首座標から 25% 幅の正方形 ROI を算出
   - 画像境界にクランプ
2. `crop_image(image, x, y, w, h) -> DynamicImage` 追加
3. `HolisticTracker::detect()` で Pose 2D ランドマーク index 15/16 から ROI 算出 → クロップ → Hand 推論
4. Pose 未検出時は従来通り全フレームでフォールバック
5. 単体テスト 4 件追加 (center ROI, edge clamping x2, crop validation)
