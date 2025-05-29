# Phase 4: Layer 2 — VTable ロード

## 実行日時
2026-03-23 01:40 JST

## 作業内容

### Steps 4-1~4-3: vtable.rs
- `VTableValidate` unsafe trait 定義
- `PluginVTable` に対する VTableValidate 実装
- `LoadedLibrary::vtable::<V>()` メソッド実装
- `lib.rs` に `pub mod vtable;` + `pub use vtable::VTableValidate;` 追加

### Step 4-4: 統合テスト
- `tests/integration.rs` 作成 (10テスト)
- 正常系: load_and_bind_entry, vtable_load, invoke_greet, invoke_add, invoke_noop
- 異常系: unknown_method(-1), panic(-2), nonexistent_file, nonexistent_symbol, wrong_entry

### Step 4-5: 品質ゲート
```bash
cargo build -p dynplug-example && cargo test -p dynplug   # 27 tests passed (17 unit + 10 integration)
cargo clippy -p dynplug -p dynplug-example -- -D warnings  # OK
cargo build -p dynplug -p dynplug-example                   # OK
```

## 結果
全ステップ完了。27テスト全パス。
