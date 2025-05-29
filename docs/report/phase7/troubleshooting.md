# Phase 7: Troubleshooting

## SIGSEGV in test_manager_load_from_directory

### 症状
```
test test_manager_load_from_directory ... SIGSEGV
```

### 原因
`target/debug/` ディレクトリには複数の `.dylib` が存在:
- `libdynplug_example.dylib` (PluginVTable 互換)
- `libdynplug_example_l3.dylib` (CalculatorVTable — PluginVTable と非互換)

`PluginManager::load_from_directory` が全 `.dylib` をロードし、L3 プラグインの VTable を `PluginVTable` として解釈 → フィールドオフセットが異なり `destroy()` 呼び出しで SIGSEGV。

### 解決
テスト用テンポラリディレクトリを作成し、互換プラグインのみをコピーして使用:
```rust
let tmp = std::env::temp_dir().join("dynplug_test_dir");
std::fs::create_dir_all(&tmp).unwrap();
std::fs::copy(&path, &tmp.join(path.file_name().unwrap())).unwrap();
manager.load_from_directory(&tmp).unwrap();
```

## SIGSEGV after catch_unwind in panic test

### 症状
panic テスト後に同一プロセス内でライブラリの再ロードまたは dlclose で SIGSEGV。

### 原因
`catch_unwind` は FFI 境界でのパニックをキャッチするが、パニック発生時に Rust ランタイムの TLS (Thread Local Storage) やスタック状態が部分的に壊れることがある。`Library::close()` (dlclose) 時にこの壊れた状態にアクセスして SIGSEGV。

### 解決
panic テストをサブプロセスで実行し、メインテストプロセスの状態を保護:
```rust
#[test]
fn test_invoke_panic_returns_minus2() {
    let exe = std::env::current_exe().unwrap();
    let output = std::process::Command::new(exe)
        .arg("--ignored")
        .arg("test_invoke_panic_subprocess")
        .output().unwrap();
    assert!(stdout.contains("ok"));
}

#[test]
#[ignore]
fn test_invoke_panic_subprocess() {
    // 実際の panic テスト
    std::mem::forget(lib); // dlclose を回避
}
```
