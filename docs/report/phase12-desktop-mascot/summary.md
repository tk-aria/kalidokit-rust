# Phase 12: デスクトップマスコット — 作業報告

## 実行日時
2026-03-18 13:17-13:27 JST

## 完了タスク

### Step 12.1: RenderContext に透過モード切替
- adapter フィールド追加 (capabilities 取得用)
- set_transparent(bool) — PostMultiplied/PreMultiplied/Auto 自動選択

### Step 12.2: Scene にクリアカラーアルファ切替
- set_clear_alpha(f64) — 0.0=透過, 1.0=不透明

### Step 12.3: MascotState モジュール新規作成
- mascot.rs — enter/leave/toggle/start_drag/update_drag/end_drag
- 4 unit tests

### Step 12.4: AppState に MascotState 追加
- mascot フィールド + last_cursor_pos

### Step 12.5: KeyM 切替 + ドラッグ移動
- KeyM → mascot.toggle + set_transparent + set_clear_alpha
- MouseInput/CursorMoved → drag 移動

### Step 12.6: ウィンドウ透過対応
- with_transparent(true) で作成

### Step 12.7: UserPrefs 永続化
- mascot_mode: bool (serde default = false)
- save_prefs/init で保存復元

## 検証結果
- cargo check --workspace — OK
- cargo test — 103 tests pass (app 15 + renderer 16 + vrm 33 + solver 39)
- cargo clippy --workspace -- -D warnings — OK
- cargo fmt --check — OK
- cargo build --release — OK

## 動作確認 (macOS release)
```
mascot_mode: true
Mascot mode: ON (512x512)
render: 23 fps | video decode: 24 fps (VideoToolbox)
```
- ✅ マスコットモードで起動
- ✅ 512x512 にリサイズ
- ✅ タイトルバーなし
- ✅ 最前面表示
- ✅ VideoToolbox デコード正常
- ✅ Release ビルド正常動作

## 実行コマンド
```
cargo check --workspace
cargo test -p kalidokit-rust -p renderer -p vrm -p solver
cargo clippy --workspace -- -D warnings
cargo fmt --check
cargo build --release
# 動作確認
RUST_LOG=info ./target/release/kalidokit-rust
```
