# Phase 2: Layer 0 + 1 — LoadedLibrary + Symbol Bind

## 実行日時
2026-03-23 01:33 JST

## 作業内容

### Step 2-1: LoadedLibrary 構造体
- `crates/dynplug/src/loader.rs` 作成
- `LoadedLibrary` struct (libloading::Library ラッパー)
- `load()`, `path()` メソッド

### Step 2-2: BoundFn + Deref
- `BoundFn<'lib, F>` struct with `Deref<Target = F>`

### Step 2-3: LoadedLibrary::bind()
- `bind::<F>(name: &str) -> Result<BoundFn<'_, F>, PluginError>`
- CString 変換 + libloading::Library::get() ラッパー

### Step 2-4: 品質ゲート
```bash
cargo check -p dynplug               # OK
cargo clippy -p dynplug -- -D warnings  # OK (0 warnings)
cargo build -p dynplug                # OK
```

## 結果
全ステップ完了。loader.rs は約90行で300行制限内。
