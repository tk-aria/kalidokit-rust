# Statement of Work: dynplug

- **Crate Name:** `dynplug`
- **Date:** 2026-03-23
- **Status:** Approved
- **Prerequisites:** RFC.md を読了していること

---

## 1. Scope

Rust 用クロスプラットフォーム動的プラグインローディングライブラリの実装。
RFC に定義された 3 層 API（Symbol Bind / VTable / Safe Wrapper）と
`PluginManager` による一元管理を提供する。

**スコープ内:**
- Layer 0-3 の全実装
- `PluginManager`（複数パス/ディレクトリ対応、Drop での確実な解放）
- `export_plugin!` マクロ（汎用 VTable エクスポート）
- サンプルプラグイン（cdylib）+ ホスト側 example
- 統合テスト

**スコープ外（RFC Section 8 参照）:**
- iOS 静的リンク、WASM、ホットリロード自動監視、ネットワーク配信

---

## 2. Deliverables

### 2.1 クレート構成

```
crates/dynplug/                          # メインライブラリ
├── Cargo.toml
├── spec/
│   ├── RFC.md
│   └── SoW.md
├── src/
│   ├── lib.rs                           # Public API, re-exports
│   ├── api.rs                           # PluginVTable, INTERFACE_VERSION, PluginEntryFn
│   ├── error.rs                         # PluginError enum
│   ├── platform.rs                      # lib_extension(), lib_prefix(), lib_filename()
│   ├── loader.rs                        # LoadedLibrary, BoundFn (Layer 0 + 1)
│   ├── vtable.rs                        # VTableValidate trait, vtable() (Layer 2)
│   ├── export.rs                        # export_plugin! macro (plugin side)
│   ├── define.rs                        # define_plugin! macro (Layer 3)
│   └── manager.rs                       # PluginManager
├── examples/
│   └── host.rs                          # ホスト側動作確認
└── tests/
    └── integration.rs                   # 統合テスト

crates/dynplug-example/                  # サンプルプラグイン (cdylib)
├── Cargo.toml
└── src/
    └── lib.rs
```

### 2.2 Dependencies

**`crates/dynplug/Cargo.toml`:**

```toml
[package]
name = "dynplug"
version = "0.1.0"
edition = "2021"
description = "Cross-platform dynamic plugin loading with layered abstraction"

[dependencies]
thiserror = { workspace = true }
log = { workspace = true }
libloading = "0.8"
```

**`crates/dynplug-example/Cargo.toml`:**

```toml
[package]
name = "dynplug-example"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
dynplug = { path = "../dynplug" }
```

**ワークスペース `Cargo.toml`:**

`members` に `"crates/dynplug"`, `"crates/dynplug-example"` を追加。

---

## 3. Implementation Phases

### Phase 1: Foundation

| Task | File | 詳細 |
|------|------|------|
| 1-1 | `Cargo.toml` (両方) | クレート作成、ワークスペースに追加 |
| 1-2 | `error.rs` | RFC 5.2 の `PluginError` enum をそのまま実装 |
| 1-3 | `platform.rs` | RFC 5.3 の 3 関数を実装 |
| 1-4 | `api.rs` | RFC 5.1 の `PluginVTable`, `INTERFACE_VERSION`, `PLUGIN_ENTRY_SYMBOL`, `PluginEntryFn` を実装 |
| 1-5 | `lib.rs` | モジュール宣言のみ（`pub mod api; pub mod error; pub mod platform;`） |

**完了条件:**
- `cargo check -p dynplug` がエラー・警告なしで通る
- `cargo check -p dynplug-example` がエラーなしで通る（中身は空の lib.rs でよい）

---

### Phase 2: Layer 0 + 1（LoadedLibrary + Symbol Bind）

| Task | File | 詳細 |
|------|------|------|
| 2-1 | `loader.rs` | `LoadedLibrary` 構造体定義。`load()` は `libloading::Library::new()` を呼び `PluginError::Load` に変換。`path()` メソッド |
| 2-2 | `loader.rs` | `BoundFn<'lib, F>` 構造体。`Deref<Target = F>` を実装。RFC 5.4 参照 |
| 2-3 | `loader.rs` | `LoadedLibrary::bind::<F>(name: &str)` — `lib.get()` を呼び `PluginError::SymbolNotFound` に変換。戻り値は `BoundFn<'_, F>` |
| 2-4 | `lib.rs` | `pub mod loader;` 追加 + `pub use` で `LoadedLibrary`, `BoundFn` を公開 |

**テストケース（`loader.rs` 内 `#[cfg(test)]`）:**

Layer 1 のテストにはプラグインバイナリが必要。Phase 3 の後にテストを書くか、
テスト用に `std::env::current_exe()` を自身に対して load して既知シンボル（例: `rust_eh_personality` や テスト用に export した関数）をバインドする。

→ **判断:** Phase 3 完了後に Phase 2 のテストを書く（Phase 6 の integration.rs で統合テスト）

**完了条件:**
- `cargo check -p dynplug` が通る
- `LoadedLibrary::load`, `bind` のコードが存在し、型が正しい

---

### Phase 3: Plugin Export + Sample Plugin

| Task | File | 詳細 |
|------|------|------|
| 3-1 | `export.rs` | `export_plugin!` マクロ。RFC 5.6 の展開例の通りに実装。以下の C ABI ブリッジ関数を生成: `__dynplug_name`, `__dynplug_version`, `__dynplug_invoke` (catch_unwind 付き), `__dynplug_free_buffer`, `__dynplug_destroy`, `__DYNPLUG_VTABLE` (static), `plugin_entry` (#[no_mangle]) |
| 3-2 | `dynplug-example/src/lib.rs` | `export_plugin!` を使ってサンプルプラグインを実装 |

**`dynplug-example/src/lib.rs` の完全な実装:**

```rust
dynplug::export_plugin! {
    name: "greeter",
    version: 1,
    invoke: handle_invoke,
}

fn handle_invoke(method: &str, input: &[u8]) -> Result<Vec<u8>, String> {
    match method {
        "greet" => {
            let name = std::str::from_utf8(input).unwrap_or("world");
            Ok(format!("Hello, {name}!").into_bytes())
        }
        "add" if input.len() == 8 => {
            let a = i32::from_le_bytes(input[..4].try_into().unwrap());
            let b = i32::from_le_bytes(input[4..8].try_into().unwrap());
            Ok((a + b).to_le_bytes().to_vec())
        }
        "panic_test" => {
            panic!("intentional panic for testing");
        }
        _ => Err(format!("unknown method: {method}")),
    }
}
```

**完了条件:**
- `cargo build -p dynplug-example` で以下のファイルが生成される:
  - macOS: `target/debug/libdynplug_example.dylib`
  - Linux: `target/debug/libdynplug_example.so`
  - Windows: `target/debug/dynplug_example.dll`

---

### Phase 4: Layer 2（VTable）

| Task | File | 詳細 |
|------|------|------|
| 4-1 | `vtable.rs` | `VTableValidate` unsafe trait を定義。RFC 5.5 参照。Safety 条件をドキュメントに記載 |
| 4-2 | `vtable.rs` | `LoadedLibrary::vtable::<V>(entry_symbol: Option<&str>)` を実装。RFC 5.5 の処理フローの通り: (1) シンボル取得 (2) null チェック (3) バージョンチェック (4) &'static V として返す |
| 4-3 | `vtable.rs` | `PluginVTable` に対する `VTableValidate` 実装（`api.rs` 内 または `vtable.rs` 内） |
| 4-4 | `lib.rs` | `pub mod vtable;` + `pub use VTableValidate` |

**完了条件:**
- `cargo check -p dynplug` が通る
- `LoadedLibrary` に `vtable()` メソッドが存在する

---

### Phase 5: PluginManager

| Task | File | 詳細 |
|------|------|------|
| 5-1 | `manager.rs` | `ManagedPlugin` 内部構造体 + `PluginManager` 構造体 + `new()` |
| 5-2 | `manager.rs` | `load_file()` — RFC 5.7 のプラグイン名取得ロジックに従う: (1) entry 試行 → VTable::name() (2) 失敗時はファイルステムから推測 (3) 重複名チェック |
| 5-3 | `manager.rs` | `load_from_directory()` — `platform::lib_extension()` でフィルタ、非再帰、個別エラーは warn してスキップ |
| 5-4 | `manager.rs` | `load_from_directories()` — `load_from_directory` の繰り返し |
| 5-5 | `manager.rs` | `load_paths()` — `Path::is_dir()` で分岐。存在しないパスは warn してスキップ |
| 5-6 | `manager.rs` | `get()`, `names()`, `plugins()` — `name_index` HashMap で O(1) 名前引き |
| 5-7 | `manager.rs` | `unload()` — (1) destroy() 呼び出し (2) LoadedLibrary drop (3) name_index 除去 (4) libraries[i] = None |
| 5-8 | `manager.rs` | `unload_all()` — libraries をロード順の**逆順**でイテレート、各 ManagedPlugin を destroy → drop |
| 5-9 | `manager.rs` | `Drop for PluginManager` — `self.unload_all()` を呼ぶ |

**PluginManager 内部の名前取得ロジック詳細（5-2）:**

```text
load_file(path) {
    lib = LoadedLibrary::load(path)?;

    // VTable からプラグイン名を取得する試行
    match lib.vtable::<PluginVTable>(None) {
        Ok(vt) => {
            name = CStr::from_ptr((vt.name)()).to_str()  // VTable::name()
            vtable = Some(vt)
        }
        Err(_) => {
            // Layer 1 専用プラグイン（VTable なし）
            // ファイル名からプラグイン名を推測
            // "libgreeter.dylib" → strip prefix "lib" → strip extension → "greeter"
            // "greeter.dll" → strip extension → "greeter"
            name = derive_name_from_path(path)
            vtable = None
        }
    }

    if name_index.contains_key(&name) {
        return Err(PluginError::DuplicateName(name))
    }

    // 登録
    let idx = libraries.len();
    libraries.push(Some(ManagedPlugin { library: lib, vtable, name: name.clone() }));
    name_index.insert(name, idx);
    Ok(&libraries[idx].as_ref().unwrap().library)
}
```

**完了条件:**
- `PluginManager::load_file()` → `get()` → `unload()` のフローが動作
- `PluginManager` の `Drop` で全プラグインが解放される

---

### Phase 6: Example Host + Integration Test

| Task | File | 詳細 |
|------|------|------|
| 6-1 | `examples/host.rs` | 下記の完全な実装 |
| 6-2 | `tests/integration.rs` | 下記のテストケース |

**`examples/host.rs` の完全な実装:**

```rust
use dynplug::{LoadedLibrary, PluginManager, PluginError};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lib_path = find_plugin_path();
    println!("=== dynplug Host Example ===");
    println!("Plugin path: {}", lib_path.display());

    // ========================================
    // Layer 1: Symbol Bind
    // ========================================
    println!("\n--- Layer 1: Symbol Bind ---");
    {
        let lib = LoadedLibrary::load(&lib_path)?;

        // export_plugin! が生成した個別のブリッジ関数は #[no_mangle] ではないため
        // Layer 1 では plugin_entry シンボルのみバインド可能。
        // Layer 1 の主な用途は、dynplug を使っていない外部 C ライブラリの読み込み。
        //
        // ここでは plugin_entry を bind してから VTable 経由で invoke を呼ぶデモ。
        let entry = lib.bind::<dynplug::PluginEntryFn>("plugin_entry")?;
        let vtable = unsafe { &*entry() };
        let name = unsafe {
            std::ffi::CStr::from_ptr((vtable.name)()).to_str().unwrap()
        };
        println!("Plugin name (via bind): {name}");
        println!("Plugin version: {}", (vtable.version)());
    }

    // ========================================
    // Layer 2: VTable
    // ========================================
    println!("\n--- Layer 2: VTable ---");
    {
        let lib = LoadedLibrary::load(&lib_path)?;
        let vt = lib.vtable::<dynplug::PluginVTable>(None)?;

        // invoke: greet
        let input = b"World";
        let mut out_ptr: *mut u8 = std::ptr::null_mut();
        let mut out_len: usize = 0;
        let rc = (vt.invoke)(
            b"greet".as_ptr(), 5,
            input.as_ptr(), input.len(),
            &mut out_ptr, &mut out_len,
        );
        assert_eq!(rc, 0);
        let greeting = unsafe {
            String::from_utf8_lossy(std::slice::from_raw_parts(out_ptr, out_len)).to_string()
        };
        (vt.free_buffer)(out_ptr, out_len);
        println!("greet(\"World\") = {greeting}");
        assert_eq!(greeting, "Hello, World!");

        // invoke: add
        let mut add_input = Vec::new();
        add_input.extend_from_slice(&21_i32.to_le_bytes());
        add_input.extend_from_slice(&21_i32.to_le_bytes());
        let rc = (vt.invoke)(
            b"add".as_ptr(), 3,
            add_input.as_ptr(), add_input.len(),
            &mut out_ptr, &mut out_len,
        );
        assert_eq!(rc, 0);
        let sum = i32::from_le_bytes(
            unsafe { std::slice::from_raw_parts(out_ptr, out_len) }
                .try_into().unwrap()
        );
        (vt.free_buffer)(out_ptr, out_len);
        println!("add(21, 21) = {sum}");
        assert_eq!(sum, 42);

        // invoke: unknown method → error (-1)
        let rc = (vt.invoke)(
            b"unknown".as_ptr(), 7,
            std::ptr::null(), 0,
            &mut out_ptr, &mut out_len,
        );
        assert_eq!(rc, -1);
        let err_msg = unsafe {
            String::from_utf8_lossy(std::slice::from_raw_parts(out_ptr, out_len)).to_string()
        };
        (vt.free_buffer)(out_ptr, out_len);
        println!("unknown method → error: {err_msg}");

        // invoke: panic → -2
        let rc = (vt.invoke)(
            b"panic_test".as_ptr(), 10,
            std::ptr::null(), 0,
            &mut out_ptr, &mut out_len,
        );
        assert_eq!(rc, -2);
        println!("panic_test → caught (rc={rc})");

        (vt.destroy)();
    }

    // ========================================
    // PluginManager
    // ========================================
    println!("\n--- PluginManager ---");
    {
        let mut manager = PluginManager::new();

        // ファイル単体ロード
        manager.load_file(&lib_path)?;
        println!("Loaded plugins: {:?}", manager.names());

        // 名前引き
        let p = manager.get("greeter").expect("greeter not found");
        println!("get(\"greeter\"): path={}", p.path().display());

        // アンロード
        manager.unload("greeter")?;
        assert!(manager.get("greeter").is_none());
        println!("unload(\"greeter\"): OK");

        // ディレクトリスキャン
        let plugin_dir = lib_path.parent().unwrap();
        let count = manager.load_from_directory(plugin_dir)?;
        println!("load_from_directory: {count} plugin(s)");

        // drop で全解放
    }
    println!("PluginManager dropped (all plugins released).");

    println!("\n=== All checks passed! ===");
    Ok(())
}

/// cargo build の出力ディレクトリからプラグインパスを見つける
fn find_plugin_path() -> PathBuf {
    // examples は target/debug/examples/ に配置されるので、
    // target/debug/ に cdylib がある
    let exe = std::env::current_exe().unwrap();
    let target_dir = exe.parent().unwrap().parent().unwrap(); // target/debug/
    target_dir.join(dynplug::lib_filename("dynplug-example"))
}
```

**`tests/integration.rs` のテストケース:**

```rust
// テスト実行前提: cargo build -p dynplug-example が完了していること
// (cargo test はデフォルトで依存クレートをビルドするが、cdylib は
//  ビルドされない場合がある。CI では明示的にビルドステップを入れる。)

fn plugin_path() -> PathBuf {
    // integration test は target/debug/deps/ から実行されるため、
    // target/debug/ に cdylib がある
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..").join("..").join("target").join("debug")
        .join(dynplug::lib_filename("dynplug-example"))
}

#[test]
fn test_load_and_bind_entry() {
    // LoadedLibrary::load + bind で plugin_entry を取得できる
}

#[test]
fn test_vtable_load_and_version_check() {
    // vtable() でバージョンチェックが通る
    // name() が "greeter" を返す
    // version() が 1 を返す
}

#[test]
fn test_invoke_greet() {
    // invoke("greet", b"World") → 0, "Hello, World!"
    // free_buffer で解放
}

#[test]
fn test_invoke_add() {
    // invoke("add", [21_i32.to_le_bytes(), 21_i32.to_le_bytes()].concat()) → 0, 42
}

#[test]
fn test_invoke_unknown_method_returns_error() {
    // invoke("unknown", &[]) → -1, エラーメッセージが取得できる
    // free_buffer でエラーメッセージを解放
}

#[test]
fn test_invoke_panic_returns_minus2() {
    // invoke("panic_test", &[]) → -2
    // out_ptr/out_len に触らない
}

#[test]
fn test_invoke_empty_output() {
    // invoke が成功し出力が空の場合、out_ptr = null, out_len = 0
    // free_buffer を呼ぶ必要がない
    // → サンプルプラグインに "noop" メソッドの追加が必要
}

#[test]
fn test_load_nonexistent_file() {
    // LoadedLibrary::load("/nonexistent") → PluginError::Load
}

#[test]
fn test_bind_nonexistent_symbol() {
    // lib.bind::<...>("no_such_symbol") → PluginError::SymbolNotFound
}

#[test]
fn test_manager_load_file_and_get() {
    // load_file → get("greeter") → Some
}

#[test]
fn test_manager_duplicate_name() {
    // 同じファイルを2回 load_file → 2回目が PluginError::DuplicateName
}

#[test]
fn test_manager_unload() {
    // load_file → unload("greeter") → get("greeter") = None
}

#[test]
fn test_manager_unload_nonexistent() {
    // unload("no_such") → PluginError::NotFound
}

#[test]
fn test_manager_load_from_directory() {
    // ディレクトリからスキャン → 少なくとも1つロードされる
}

#[test]
fn test_manager_load_paths_mixed() {
    // ファイルパスとディレクトリパスを混在させて渡す
}

#[test]
fn test_manager_drop_releases_all() {
    // PluginManager を drop して、再度同じファイルを load できることを確認
    // (Library が close されていないと OS が同じパスのロードをキャッシュする可能性がある)
}
```

**完了条件:**
- `cargo build -p dynplug-example && cargo run -p dynplug --example host` が全 assert 通過
- `cargo build -p dynplug-example && cargo test -p dynplug` で全テスト通過

---

### Phase 7: Layer 3（Safe Wrapper Macro）

| Task | File | 詳細 |
|------|------|------|
| 7-1 | `define.rs` | `define_plugin!` マクロ。RFC 5.8 の生成コード仕様に従う |
| 7-2 | `define.rs` | FFI 型変換: プリミティブ型（i32 等）はそのまま。`&str` → ptr+len。`String` → out params。`Result` → return code + out params。RFC 5.8 の型変換テーブル参照 |
| 7-3 | `define.rs` | プラグイン側エクスポートマクロ (`export_{name}!`) の自動生成。catch_unwind 付き |
| 7-4 | テスト | `define_plugin!` で Greeter を定義し、ホスト側ラッパー経由で `greeter.add(21, 21)` が 42 を返す |

**`define_plugin!` の制約（v0.1）:**

`macro_rules!` で実装するため、以下の制約がある:
- 関数の引数型・戻り値型は事前定義されたパターンのみ対応（RFC 5.8 の型変換テーブル）
- ジェネリクス、ライフタイムパラメータは非対応
- これらの制約が問題になった場合、v0.2 で proc macro (`dynplug-macros` クレート) に移行する

**Phase 7 着手の前提条件:**
- Phase 1-6 が全て完了し、Layer 1/2 + PluginManager が安定していること
- Phase 7 で発生する問題が Layer 1/2 の設計変更を要求しないことを確認

**完了条件:**
- `define_plugin!` で定義した `Greeter` 構造体のメソッド呼び出しで、サンプルプラグインの関数が正しく動作する
- Layer 3 用の統合テストが通る

---

## 4. Decision Log

実装中に判断が必要になる可能性がある項目と、その判断基準。

| # | 判断項目 | デフォルト判断 | 変更する場合の条件 |
|---|---------|--------------|------------------|
| D-1 | `BoundFn` の呼び出しを safe にするか unsafe にするか | safe（`Deref<Target = F>` で透過的に呼べる） | 型パラメータの不一致による UB が問題になった場合は unsafe に変更 |
| D-2 | `vtable()` の戻り値 `&'static V` のライフタイム | `'static`（プラグインの static メモリを指す） | ライフタイムを `&'lib V` に変更する場合は LoadedLibrary にライフタイムパラメータを追加 |
| D-3 | `PluginManager` の内部データ構造 | `Vec<Option<ManagedPlugin>>` + `HashMap<String, usize>` | パフォーマンスが問題になった場合は `SlotMap` 等に変更 |
| D-4 | `export_plugin!` の `destroy` をカスタマイズ可能にするか | v0.1 ではデフォルト（no-op）のみ | 要望があれば `destroy: my_cleanup` のオプションフィールドを追加 |
| D-5 | `define_plugin!` を `macro_rules!` で実装するか proc macro にするか | `macro_rules!` | 型変換パターンが複雑になった場合は proc macro に移行し、`dynplug-macros` クレートを追加 |

---

## 5. Build & Test Commands

```bash
# Phase 1: 型チェック
cargo check -p dynplug
cargo check -p dynplug-example

# Phase 3: プラグインビルド
cargo build -p dynplug-example

# Phase 6: example 実行
cargo build -p dynplug-example && cargo run -p dynplug --example host

# 全テスト
cargo build -p dynplug-example && cargo test -p dynplug

# Lint
cargo clippy -p dynplug -p dynplug-example -- -D warnings

# Format
cargo fmt -p dynplug -p dynplug-example --check
```

**注意:** `cargo test -p dynplug` は `tests/integration.rs` が `dynplug-example` の cdylib を読み込むため、
先に `cargo build -p dynplug-example` が必要。CI では明示的にビルドステップを分ける。

---

## 6. Acceptance Criteria

| # | 基準 | 検証方法 |
|---|------|---------|
| AC-1 | `cargo check -p dynplug` が警告なしで通る | CI |
| AC-2 | `cargo clippy -p dynplug -- -D warnings` が通る | CI |
| AC-3 | `cargo build -p dynplug-example` で cdylib が生成される | CI |
| AC-4 | `cargo run -p dynplug --example host` が全 assert を通過する | CI + 手動 |
| AC-5 | `cargo test -p dynplug` で全テスト通過 | CI |
| AC-6 | Layer 1: `bind` でシンボル取得 + 関数呼び出し成功 | AC-4, AC-5 |
| AC-7 | Layer 2: `vtable` で VTable 取得 + invoke 呼び出し成功 | AC-4, AC-5 |
| AC-8 | invoke エラー: unknown method → -1 + エラーメッセージ取得 | AC-5 |
| AC-9 | invoke パニック: → -2、ホストがクラッシュしない | AC-5 |
| AC-10 | PluginManager: 複数パスロード + 名前引き + アンロード | AC-5 |
| AC-11 | PluginManager: 同名プラグインの重複ロードがエラーになる | AC-5 |
| AC-12 | PluginManager: Drop で全プラグインが確実に解放される | AC-5 |
| AC-13 | エラーケース: 存在しないパス → `PluginError::Load` | AC-5 |
| AC-14 | エラーケース: 存在しないシンボル → `PluginError::SymbolNotFound` | AC-5 |
