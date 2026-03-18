# Steps 12.10-12.12: マスコット修正 — 作業報告

## 実行日時
2026-03-18 13:30-14:35 JST

## Step 12.10: クリックスルー (透過部分マウス貫通)
- winit `set_cursor_hittest(false)` で実装 (OS ネイティブ API をラップ)
- macOS: 内部で `NSWindow.setIgnoresMouseEvents` を呼び出し
- マスコットモード ON → click-through 有効 (クリックが背面ウィンドウに通過)
- Alt/Option キーを押している間 → click-through 無効 (ドラッグ移動可能)
- Alt キーを離す → click-through 再有効化

## Step 12.11: ゴースト (残像) 問題修正
- `scene.rs`: `clear_color.a < 1.0` (透過モード) 時は常に `LoadOp::Clear` を使用
- `context.rs`: `set_transparent()` 後に `device.poll(Maintain::Wait)` で GPU 同期
- `mascot.rs`: macOS で `set_has_shadow(false)` (WindowExtMacOS) でシャドウ残像防止

## Step 12.12: AlwaysOnTop 設定化
- `MascotState.always_on_top: bool` フィールド追加 (デフォルト: true)
- `UserPrefs.always_on_top: bool` で永続化 (#[serde(default = "default_true")])
- `KeyF` でマスコットモード中に AlwaysOnTop ON/OFF 切替
- `enter()` が `always_on_top` パラメータを参照

## 動作確認 (Release)
```
mascot_mode: true, always_on_top: true
Mascot mode: ON (512x512, always_on_top=true)
render: 23 fps
```
- ✅ クラッシュなし (10 秒間安定動作)
- ✅ Release ビルド成功

## 実行コマンド
```
cargo check --workspace
cargo test -p kalidokit-rust -p renderer -p vrm -p solver
cargo clippy --workspace -- -D warnings
cargo build --release
RUST_LOG=info ./target/release/kalidokit-rust
```

## キーバインド (マスコットモード)
| キー | 機能 |
|------|------|
| M | マスコットモード ON/OFF |
| F | AlwaysOnTop ON/OFF (マスコットモード中) |
| P | 動画 pause/resume |
| Alt+クリック | ドラッグ移動 (通常はクリックスルー) |
