# Phase 7: Layer 3 — define_plugin! マクロ

## 実行日時
2026-03-23 01:55 JST

## 作業内容

### Step 7-1: define_plugin! マクロ
- `crates/dynplug/src/define.rs` 作成
- `paste = "1.0"` 依存追加（識別子結合用）
- マクロ生成物: `<Name>VTable` (#[repr(C)]), `VTableValidate` impl, `<Name>` ラッパー + `load()` + メソッド + `Drop`
- v0.1: プリミティブ型のみ対応。&str/String は Layer 2 直接使用を推奨

### Step 7-2: ホスト側ラッパー
- `Calculator::load(path)` → `LoadedLibrary::load` + `vtable::<CalculatorVTable>(None)`
- `calc.add(21, 21)` → `(self.vtable.add)(21, 21)` のように透過的に呼び出し

### Step 7-3: プラグイン側エクスポート
- v0.1 では手動 VTable 定義。v0.2 で proc macro 移行予定

### Step 7-4: テスト (tests/layer3.rs)
- 6テスト: load, add, multiply, negate, drop, load_nonexistent

### Step 7-5: 品質ゲート
```bash
cargo build -p dynplug-example -p dynplug-example-l3  # OK
cargo test -p dynplug                                   # 44 tests (17 unit + 21 integration + 6 layer3)
cargo clippy -p dynplug -p dynplug-example -p dynplug-example-l3 -- -D warnings  # OK
cargo fmt -p dynplug --check                            # OK
```

## トラブルシューティング
### SIGSEGV in test_manager_load_from_directory
- 原因: `target/debug/` ディレクトリに L3 プラグイン (CalculatorVTable) が存在し、PluginManager が PluginVTable として解釈 → メモリレイアウト不一致で SIGSEGV
- 解決: テスト用テンポラリディレクトリに dynplug-example のみコピーして使用

### SIGSEGV after catch_unwind in panic test
- 原因: catch_unwind 後のライブラリ TLS 状態破損で dlclose 時にクラッシュ
- 解決: panic テストをサブプロセスで実行、ライブラリハンドルを mem::forget で leak

## 結果
全ステップ完了。44テスト全パス。
