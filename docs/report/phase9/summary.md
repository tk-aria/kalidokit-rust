# Phase 9: Extism Backend — Wasm 多言語プラグイン対応

## 実行日時
2026-03-23 02:30 JST

## 作業内容

### Step 9-1: PluginBackend trait 抽出
- `crates/dynplug/src/backend.rs` 作成
- `PluginBackend` trait: `name()`, `invoke()`, `path()`, `kind()`, `as_any()`
- native/wasm 両バックエンドの共通インターフェース

### Step 9-2: NativeBackend
- `crates/dynplug/src/native.rs` 作成
- 既存の `LoadedLibrary` + `PluginVTable` をラップ
- `invoke()` で VTable 経由の呼び出しを実装 (rc: 0=成功, -1=エラー, -2=パニック)
- `Drop` で `vtable.destroy()` を自動呼び出し

### Step 9-3: WasmBackend (Extism)
- `crates/dynplug/src/wasm.rs` 作成 (`#[cfg(feature = "wasm")]` ゲート)
- `extism::Plugin` をラップ
- `load()`, `load_with_wasi()`, `load_manifest()` の 3 ロードパス
- `invoke()` は `plugin.call::<&[u8], Vec<u8>>()` にデリゲート

### Step 9-4: manager.rs リファクタ
- `ManagedPlugin` 内部を `Box<dyn PluginBackend>` に変更
- 新規メソッド: `load_wasm()`, `load_wasm_manifest()`, `invoke()`, `plugin_kind()`
- `load_from_directory()` が `.wasm` もスキャン (wasm feature 有効時)
- 既存 API (`load_file`, `get`, `names`, `plugins`, `unload`) の互換性維持

### Step 9-5: Feature Flag
- `[features] wasm = ["extism"]` — opt-in (Wasmtime ~20MB のため)
- `extism = { version = "1.20", optional = true }`
- wasm feature 無効時は既存コードと完全互換

### Step 9-6: Wasm サンプルプラグイン
- `crates/dynplug-example-wasm/` 作成 (extism-pdk, crate-type = cdylib)
- `greet`, `add`, `noop` メソッド実装
- `cargo build -p dynplug-example-wasm --target wasm32-unknown-unknown` → OK

### Step 9-7: テスト
- 16 テスト追加 (`tests/wasm_integration.rs`)
- WasmBackend 直接: load, name, invoke_greet, invoke_add, invoke_noop, invoke_unknown, load_nonexistent, kind
- PluginManager: load_wasm, invoke_wasm, invoke_native, mixed_plugins, plugin_kind, unload_wasm, duplicate_name, directory_mixed

### Step 9-8: 品質ゲート
```bash
cargo test -p dynplug                       # 44 tests (既存全パス)
cargo test -p dynplug --features wasm       # 60 tests (既存 + 16 wasm)
cargo clippy -p dynplug --features wasm -- -D warnings  # OK
cargo clippy -p dynplug -- -D warnings                  # OK (wasm なし)
cargo fmt -p dynplug --check                             # OK
```

## アーキテクチャ

```
PluginManager
  ├── NativeBackend  (cdylib: Rust/C, Layer 1-3)
  │     └── LoadedLibrary + PluginVTable
  └── WasmBackend    (Extism: Rust/Go/JS/Python/C/Zig...)
        └── extism::Plugin (Wasmtime)
```

## 結果
全 60 テストパス。既存 44 テストにリグレッションなし。
