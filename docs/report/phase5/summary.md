# Phase 5: PluginManager

## 実行日時
2026-03-23 01:45 JST

## 作業内容

### Steps 5-1~5-5: manager.rs
- `ManagedPlugin` 内部構造体 (library + vtable)
- `PluginManager` struct (HashMap + Vec)
- `new()`, `load_file()`, `load_from_directory()`, `load_from_directories()`, `load_paths()`
- `get()`, `names()`, `plugins()`
- `unload()`, `unload_all()`, `Drop`, `Default`
- `derive_name_from_path()` ヘルパー

### Step 5-6: テスト (11件)
- 正常系: load_file_and_get, names, plugins, unload, load_from_directory, load_paths_mixed, drop_releases_all
- 異常系: duplicate_name, unload_nonexistent, load_from_nonexistent_directory, load_paths_nonexistent_skipped

### Step 5-7: 品質ゲート
```bash
cargo build -p dynplug-example && cargo test -p dynplug   # 38 tests passed (17 unit + 21 integration)
cargo clippy -p dynplug -p dynplug-example -- -D warnings  # OK
cargo fmt -p dynplug                                        # OK
cargo build -p dynplug -p dynplug-example                   # OK
```

## 結果
全ステップ完了。manager.rs は251行で300行制限内。
