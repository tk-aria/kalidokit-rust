# Step 12.14: ピクセルアルファベースのヒットテスト — 作業報告

## 実行日時
2026-03-18 14:36-14:47 JST

## 実装内容

### アルファマップによる動的ヒットテスト
- 毎フレーム、既存の GPU readback パイプライン (仮想カメラと共通) を使って BGRA フレームを取得
- alpha チャンネル (byte offset 3) を抽出して `mascot_alpha_map: Vec<u8>` にキャッシュ
- `CursorMoved` イベントでカーソル位置のアルファ値を O(1) で参照
- alpha > 0 → `set_cursor_hittest(true)` (操作可能: ドラッグ、スクロール、クリック)
- alpha = 0 → `set_cursor_hittest(false)` (クリックスルー: 背面ウィンドウへ)

### 変更ファイル
- `state.rs`: `mascot_alpha_map`, `mascot_alpha_width`, `mascot_alpha_height` フィールド追加
- `init.rs`: alpha map フィールド初期化
- `update.rs`: フレーム毎に `render_to_capture` → alpha 抽出 (ステップ7追加)
- `app.rs`: `CursorMoved` でアルファベースヒットテスト、`ModifiersChanged` Alt 制御を廃止
- `mascot.rs`: `set_click_through()` メソッド削除 (不要に)

### 動作原理
```
render_to_view → present (画面表示)
                → render_to_capture → capture_frame_async → BGRA readback
                  → alpha 抽出 → mascot_alpha_map にキャッシュ
                    → CursorMoved で alpha[x,y] 参照 → set_cursor_hittest
```

### HiDPI 対応
- カーソル座標 (physical pixels) をスケールファクタで除算して logical pixels に変換
- alpha map は logical size (512x512) で保持

## 検証結果
- Release ビルド成功
- 10 秒間クラッシュなし、23 fps 安定
- Alt キー不要で直感的操作

## 実行コマンド
```
cargo check --workspace
cargo test -p kalidokit-rust -p renderer
cargo clippy --workspace -- -D warnings
cargo build --release
RUST_LOG=info ./target/release/kalidokit-rust
```
