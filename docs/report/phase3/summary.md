# Phase 3: Plugin Export + サンプルプラグイン

## 実行日時
2026-03-23 01:35 JST

## 作業内容

### Step 3-1: export_plugin! マクロ
- `crates/dynplug/src/export.rs` 作成
- `export_plugin!` マクロ: name, version, invoke の3引数
- 生成コード: name(), version(), invoke() (catch_unwind付き), free_buffer(), destroy(), static VTABLE, plugin_entry()
- `lib.rs` に `pub mod export;` 追加

### Step 3-2: サンプルプラグイン
- `crates/dynplug-example/src/lib.rs` 実装
- メソッド: greet, add, noop, panic_test, unknown → error

### Step 3-3: 品質ゲート
```bash
cargo build -p dynplug-example           # OK (libdynplug_example.dylib 494KB)
cargo clippy -p dynplug -p dynplug-example -- -D warnings  # OK
cargo fmt -p dynplug --check             # OK
cargo fmt -p dynplug-example --check     # OK
```

## 結果
全ステップ完了。cdylib 正常生成確認。
