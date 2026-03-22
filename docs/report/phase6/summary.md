# Phase 6: Example Host + 統合動作確認

## 実行日時
2026-03-23 01:48 JST

## 作業内容

### Step 6-1: examples/host.rs
- Layer 1: Symbol Bind で plugin_entry をバインドし VTable 取得
- Layer 2: VTable で greet, add, unknown(-1), panic(-2) の4パターン確認
- PluginManager: load_file → get → unload → load_from_directory

### Step 6-2: 統合テスト総合確認
```bash
cargo build -p dynplug-example && cargo run -p dynplug --example host
# Output: "=== All checks passed! ==="
```

### Step 6-3: 品質ゲート
```bash
cargo build -p dynplug-example && cargo test -p dynplug   # 38 tests passed
cargo build -p dynplug-example && cargo run -p dynplug --example host  # All assertions pass
cargo clippy -p dynplug -p dynplug-example -- -D warnings  # OK
cargo fmt -p dynplug --check                                # OK
cargo build -p dynplug -p dynplug-example                   # OK
```

## 結果
全ステップ完了。Layer 1/2/PluginManager 全て正常動作確認済み。
