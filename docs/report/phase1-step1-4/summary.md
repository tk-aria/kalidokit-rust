# Phase 1 Step 1-4: api.rs — C ABI 共通型

## 実行日時
2026-03-23 01:27 JST

## 作業内容

### 1. ファイル作成
- `crates/dynplug/src/api.rs` — `PluginVTable`, `PluginEntryFn`, `INTERFACE_VERSION`, `PLUGIN_ENTRY_SYMBOL`

### 2. lib.rs 更新
- `pub mod api;` + `pub use api::{PluginVTable, PluginEntryFn, INTERFACE_VERSION, PLUGIN_ENTRY_SYMBOL};` 追加

### 3. ビルド確認
```bash
cargo check -p dynplug  # OK
```

## 結果
エラーなしで完了。
