# dynplug — Implementation TODO

> Cross-platform dynamic plugin loading for Rust
>
> **設計書:** [spec/RFC.md](spec/RFC.md) / [spec/SoW.md](spec/SoW.md)
>
> **Dependencies:**
> - `libloading = "0.8"` — Cross-platform dlopen/LoadLibrary wrapper
> - `thiserror = "2.0"` (workspace) — Error derive macro
> - `log = "0.4"` (workspace) — Logging facade
> - Rust edition: 2021

---

## Phase 1: Foundation — クレート作成と基盤型

### Step 1-1: クレート作成 + ワークスペース登録

- [x] `crates/dynplug/Cargo.toml` を作成 <!-- 2026-03-23 01:25 JST -->

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

[dev-dependencies]
env_logger = "0.11"
```

- [x] `crates/dynplug-example/Cargo.toml` を作成 <!-- 2026-03-23 01:25 JST -->

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

- [x] ワークスペースルートの `Cargo.toml` の `members` に `"crates/dynplug"`, `"crates/dynplug-example"` を追加 <!-- 2026-03-23 01:25 JST -->
- [x] `crates/dynplug/src/lib.rs` を空ファイルで作成 <!-- 2026-03-23 01:25 JST -->
- [x] `crates/dynplug-example/src/lib.rs` を空ファイルで作成 <!-- 2026-03-23 01:25 JST -->
- [x] `cargo check -p dynplug && cargo check -p dynplug-example` が通ることを確認 <!-- 2026-03-23 01:25 JST -->

### Step 1-2: `error.rs` — エラー型定義

- [ ] `crates/dynplug/src/error.rs` を作成。RFC Section 5.2 のコードをそのまま実装する

```rust
// 参考: 全バリアントと derive, From 実装
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to load library '{path}': {source}")]
    Load { path: String, source: String },

    #[error("symbol not found: '{symbol}' in '{path}'")]
    SymbolNotFound { symbol: String, path: String },

    #[error("interface version mismatch: host expects {host}, plugin has {plugin} (library: {path})")]
    VersionMismatch { host: u32, plugin: u32, path: String },

    #[error("plugin entry returned null vtable (library: {path})")]
    NullVTable { path: String },

    #[error("plugin not found: '{0}'")]
    NotFound(String),

    #[error("plugin invoke error: {message}")]
    Invoke { message: String },

    #[error("plugin panicked during invoke")]
    Panic,

    #[error("plugin '{0}' is already loaded")]
    DuplicateName(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

- [ ] `lib.rs` に `pub mod error;` と `pub use error::PluginError;` を追加

### Step 1-3: `platform.rs` — プラットフォーム抽象

- [ ] `crates/dynplug/src/platform.rs` を作成。以下の 3 関数を実装する（RFC Section 5.3）

```rust
/// "so" / "dylib" / "dll" を返す
pub fn lib_extension() -> &'static str { ... }

/// "lib" / "" を返す
pub fn lib_prefix() -> &'static str { ... }

/// クレート名 → ファイル名。ハイフンをアンダースコアに変換する
/// 例: "dynplug-example" → "libdynplug_example.dylib"
pub fn lib_filename(crate_name: &str) -> String { ... }
```

- [ ] `lib.rs` に `pub mod platform;` と `pub use platform::lib_filename;` を追加

### Step 1-4: `api.rs` — C ABI 共通型

- [ ] `crates/dynplug/src/api.rs` を作成。RFC Section 5.1 の全コードを実装する
  - `INTERFACE_VERSION: u32 = 1`
  - `PLUGIN_ENTRY_SYMBOL: &str = "plugin_entry"`
  - `#[repr(C)] pub struct PluginVTable { ... }` — 全 6 フィールド（RFC のドキュメントコメント含む）
  - `pub type PluginEntryFn = extern "C" fn() -> *const PluginVTable;`
- [ ] `lib.rs` に `pub mod api;` と `pub use api::{PluginVTable, PluginEntryFn, INTERFACE_VERSION, PLUGIN_ENTRY_SYMBOL};` を追加

### Step 1-5: Phase 1 テスト

- [ ] `platform.rs` に `#[cfg(test)] mod tests` を追加し、以下のテストを実装:
  - 正常系: `lib_extension()` が現在の OS で正しい値を返す（macOS なら `"dylib"`）
  - 正常系: `lib_prefix()` が現在の OS で正しい値を返す
  - 正常系: `lib_filename("my-plugin")` → `"libmy_plugin.dylib"` (macOS) etc.
  - 正常系: `lib_filename("simple")` → ハイフンなしのケース
  - 異常系: `lib_filename("")` → 空文字列でもパニックしない（空のファイル名が返る）
- [ ] `error.rs` に `#[cfg(test)] mod tests` を追加し、以下のテストを実装:
  - 正常系: 各 `PluginError` バリアントの `Display` 出力が期待通りか
  - 正常系: `PluginError::Io` への `From<std::io::Error>` 変換が動作するか
- [ ] `api.rs` に `#[cfg(test)] mod tests` を追加し、以下のテストを実装:
  - 正常系: `INTERFACE_VERSION` が 1 であること
  - 正常系: `PLUGIN_ENTRY_SYMBOL` が `"plugin_entry"` であること
  - 正常系: `PluginVTable` が `#[repr(C)]` であり `std::mem::size_of` が期待通りか（ポインタサイズ依存）

### Step 1-6: Phase 1 品質ゲート

- [ ] `cargo test -p dynplug` で全テスト通過
- [ ] `cargo clippy -p dynplug -p dynplug-example -- -D warnings` が通る
- [ ] `cargo fmt -p dynplug --check` が通る
- [ ] `cargo check -p dynplug && cargo check -p dynplug-example` が通る
- [ ] テストカバレッジが Phase 1 対象コード（error.rs, platform.rs, api.rs）で 90% 以上。未カバーの行があれば追加テストを書く

---

## Phase 2: Layer 0 + 1 — LoadedLibrary + Symbol Bind

### Step 2-1: `loader.rs` — LoadedLibrary 構造体

- [ ] `crates/dynplug/src/loader.rs` を作成
- [ ] `LoadedLibrary` 構造体を定義（RFC Section 5.4）

```rust
use std::path::{Path, PathBuf};

pub struct LoadedLibrary {
    lib: libloading::Library,
    path: PathBuf,
}

impl LoadedLibrary {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PluginError> {
        let path = path.as_ref();
        let lib = unsafe {
            libloading::Library::new(path)
                .map_err(|e| PluginError::Load {
                    path: path.display().to_string(),
                    source: e.to_string(),
                })?
        };
        Ok(Self { lib, path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path { &self.path }
}
```

- [ ] `lib.rs` に `pub mod loader;` と `pub use loader::LoadedLibrary;` を追加

### Step 2-2: `loader.rs` — BoundFn + Deref

- [ ] `BoundFn<'lib, F>` 構造体を `loader.rs` 内に定義（RFC Section 5.4）

```rust
pub struct BoundFn<'lib, F> {
    sym: libloading::Symbol<'lib, F>,
}

impl<F> std::ops::Deref for BoundFn<'_, F> {
    type Target = F;
    fn deref(&self) -> &F { &self.sym }
}
```

- [ ] `lib.rs` の `pub use` に `BoundFn` を追加

### Step 2-3: `loader.rs` — LoadedLibrary::bind()

- [ ] `LoadedLibrary::bind::<F>(name: &str)` メソッドを実装

```rust
impl LoadedLibrary {
    pub fn bind<F>(&self, name: &str) -> Result<BoundFn<'_, F>, PluginError> {
        let c_name = std::ffi::CString::new(name).map_err(|_| PluginError::SymbolNotFound {
            symbol: name.to_string(),
            path: self.path.display().to_string(),
        })?;
        unsafe {
            let sym = self.lib.get::<F>(c_name.as_bytes_with_nul())
                .map_err(|e| PluginError::SymbolNotFound {
                    symbol: name.to_string(),
                    path: self.path.display().to_string(),
                })?;
            Ok(BoundFn { sym })
        }
    }
}
```

> **ファイル行数見積もり:** `loader.rs` は約 80 行。300 行を超えない。

### Step 2-4: Phase 2 品質ゲート

- [ ] `cargo check -p dynplug` が通る
- [ ] `cargo clippy -p dynplug -- -D warnings` が通る
- [ ] `cargo build -p dynplug` が通る（テストは Phase 3 後に cdylib が必要なため Phase 3 後に実行）

---

## Phase 3: Plugin Export + サンプルプラグイン

### Step 3-1: `export.rs` — export_plugin! マクロ

- [ ] `crates/dynplug/src/export.rs` を作成
- [ ] `export_plugin!` マクロを実装する。RFC Section 5.6 の展開例に従い、以下の関数を生成する:

```rust
/// マクロの入力形式:
/// dynplug::export_plugin! {
///     name: "greeter",
///     version: 1,
///     invoke: handle_invoke,
/// }
///
/// 生成されるもの:
/// 1. extern "C" fn __dynplug_name() → *const c_char (null 終端文字列)
/// 2. extern "C" fn __dynplug_version() → u32
/// 3. extern "C" fn __dynplug_invoke(...) → i32
///    - catch_unwind でパニック境界
///    - Ok(buf): buf を Box::into_raw → out_ptr/out_len に書き込み → return 0
///    - Ok(empty): out_ptr=null, out_len=0 → return 0
///    - Err(msg): msg を UTF-8 バイト列で out_ptr/out_len に書き込み → return -1
///    - パニック: return -2、out_ptr/out_len は書き込まない
/// 4. extern "C" fn __dynplug_free_buffer(ptr, len)
///    - ptr が null でなく len > 0 なら Box::from_raw(slice::from_raw_parts_mut(ptr, len)) を drop
/// 5. extern "C" fn __dynplug_destroy() — no-op
/// 6. static __DYNPLUG_VTABLE: PluginVTable
/// 7. #[no_mangle] pub extern "C" fn plugin_entry() → *const PluginVTable
#[macro_export]
macro_rules! export_plugin { ... }
```

- [ ] `lib.rs` に `pub mod export;` を追加（マクロは `#[macro_export]` で自動的にクレートルートにエクスポートされる）

> **ファイル行数見積もり:** `export.rs` は約 100-120 行。300 行を超えない。

### Step 3-2: `dynplug-example/src/lib.rs` — サンプルプラグイン実装

- [ ] SoW Section 3 Phase 3 の完全なコードをそのまま実装する:

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
        "noop" => {
            Ok(Vec::new())  // 空出力テスト用
        }
        "panic_test" => {
            panic!("intentional panic for testing");
        }
        _ => Err(format!("unknown method: {method}")),
    }
}
```

### Step 3-3: Phase 3 品質ゲート

- [ ] `cargo build -p dynplug-example` で cdylib が生成されることを確認:
  - macOS: `target/debug/libdynplug_example.dylib`
  - Linux: `target/debug/libdynplug_example.so`
  - Windows: `target/debug/dynplug_example.dll`
- [ ] `cargo clippy -p dynplug -p dynplug-example -- -D warnings` が通る
- [ ] `cargo build -p dynplug` が通る

---

## Phase 4: Layer 2 — VTable ロード

### Step 4-1: `vtable.rs` — VTableValidate trait

- [ ] `crates/dynplug/src/vtable.rs` を作成
- [ ] `VTableValidate` unsafe trait を定義（RFC Section 5.5）

```rust
/// # Safety
/// この trait を実装する型は以下を満たすこと:
/// 1. `#[repr(C)]` であること
/// 2. 先頭フィールドが `interface_version: u32` であること
/// 3. 全フィールドが `extern "C" fn` 型または C ABI 互換な型であること
pub unsafe trait VTableValidate {
    fn interface_version(&self) -> u32;
}
```

### Step 4-2: `vtable.rs` — PluginVTable への VTableValidate 実装

- [ ] `api.rs` 内 または `vtable.rs` 内で `PluginVTable` に `VTableValidate` を実装

```rust
unsafe impl VTableValidate for PluginVTable {
    fn interface_version(&self) -> u32 {
        self.interface_version
    }
}
```

### Step 4-3: `vtable.rs` — LoadedLibrary::vtable()

- [ ] `LoadedLibrary::vtable::<V>(entry_symbol: Option<&str>)` を実装

```rust
impl LoadedLibrary {
    pub fn vtable<V: VTableValidate>(
        &self,
        entry_symbol: Option<&str>,
    ) -> Result<&'static V, PluginError> {
        let symbol_name = entry_symbol.unwrap_or(PLUGIN_ENTRY_SYMBOL);

        // 1. エントリーシンボルを取得 (extern "C" fn() -> *const V)
        let entry_fn = self.bind::<extern "C" fn() -> *const V>(symbol_name)?;

        // 2. エントリー関数を呼び出し
        let vtable_ptr = entry_fn();

        // 3. null チェック
        if vtable_ptr.is_null() {
            return Err(PluginError::NullVTable {
                path: self.path.display().to_string(),
            });
        }

        // 4. &'static V に変換
        let vtable = unsafe { &*vtable_ptr };

        // 5. バージョンチェック
        if vtable.interface_version() != INTERFACE_VERSION {
            return Err(PluginError::VersionMismatch {
                host: INTERFACE_VERSION,
                plugin: vtable.interface_version(),
                path: self.path.display().to_string(),
            });
        }

        Ok(vtable)
    }
}
```

- [ ] `lib.rs` に `pub mod vtable;` と `pub use vtable::VTableValidate;` を追加

> **ファイル行数見積もり:** `vtable.rs` は約 60 行。300 行を超えない。

### Step 4-4: Phase 4 テスト

- [ ] `tests/integration.rs` を作成し、cdylib を使った以下のテストを実装する:

**正常系:**
- [ ] `test_load_and_bind_entry` — `LoadedLibrary::load` + `bind::<PluginEntryFn>("plugin_entry")` が成功し、関数を呼び出して VTable ポインタが non-null
- [ ] `test_vtable_load_and_version_check` — `vtable::<PluginVTable>(None)` が成功。`name()` が `"greeter"` を返す。`version()` が 1 を返す
- [ ] `test_invoke_greet` — `invoke("greet", b"World")` → rc=0, output=`"Hello, World!"`。`free_buffer` で解放
- [ ] `test_invoke_add` — `invoke("add", 21_i32.to_le_bytes() ++ 21_i32.to_le_bytes())` → rc=0, output=42 (i32 LE)。`free_buffer` で解放
- [ ] `test_invoke_noop_empty_output` — `invoke("noop", &[])` → rc=0, out_ptr=null, out_len=0。`free_buffer` を呼ぶ必要がない

**異常系:**
- [ ] `test_invoke_unknown_method_returns_error` — `invoke("unknown", &[])` → rc=-1。out_ptr にエラーメッセージ。`free_buffer` で解放
- [ ] `test_invoke_panic_returns_minus2` — `invoke("panic_test", &[])` → rc=-2。out_ptr/out_len に触らない
- [ ] `test_load_nonexistent_file` — `LoadedLibrary::load("/nonexistent/path.dylib")` → `PluginError::Load`
- [ ] `test_bind_nonexistent_symbol` — 有効なライブラリに対して `bind::<...>("no_such_symbol")` → `PluginError::SymbolNotFound`
- [ ] `test_vtable_with_wrong_entry_symbol` — `vtable::<PluginVTable>(Some("nonexistent"))` → `PluginError::SymbolNotFound`

```rust
// テスト内で cdylib パスを取得するヘルパー:
fn plugin_path() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..").join("..").join("target").join("debug")
        .join(dynplug::lib_filename("dynplug-example"))
}
```

### Step 4-5: Phase 4 品質ゲート

- [ ] `cargo build -p dynplug-example && cargo test -p dynplug` で全テスト通過
- [ ] `cargo clippy -p dynplug -p dynplug-example -- -D warnings` が通る
- [ ] テストカバレッジが loader.rs, vtable.rs, export.rs で 90% 以上。未カバーの分岐があれば追加テストを書く
- [ ] `cargo build -p dynplug -p dynplug-example` が通る

---

## Phase 5: PluginManager

### Step 5-1: `manager.rs` — 構造体 + 内部型

- [ ] `crates/dynplug/src/manager.rs` を作成
- [ ] `ManagedPlugin` 内部構造体を定義

```rust
use std::collections::HashMap;

struct ManagedPlugin {
    library: LoadedLibrary,
    vtable: Option<&'static PluginVTable>,
    name: String,
}

pub struct PluginManager {
    name_index: HashMap<String, usize>,
    libraries: Vec<Option<ManagedPlugin>>,
}
```

- [ ] `PluginManager::new()` を実装

### Step 5-2: `manager.rs` — load_file()

- [ ] `load_file()` を実装。SoW Section 3 Phase 5 の疑似コードに従う:
  1. `LoadedLibrary::load(path)` でライブラリをロード
  2. `lib.vtable::<PluginVTable>(None)` を試行
  3. 成功: `CStr::from_ptr((vt.name)())` からプラグイン名を取得
  4. 失敗: `derive_name_from_path(path)` でファイル名から推測（`lib` プレフィックス除去 + 拡張子除去）
  5. `name_index` で重複チェック → `PluginError::DuplicateName`
  6. `ManagedPlugin` を `libraries` に追加、`name_index` に登録
  7. `&LoadedLibrary` を返す

- [ ] `derive_name_from_path()` 内部ヘルパーを実装

```rust
/// "libgreeter.dylib" → "greeter"
/// "greeter.dll" → "greeter"
fn derive_name_from_path(path: &Path) -> String {
    let stem = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let prefix = crate::platform::lib_prefix();
    if !prefix.is_empty() && stem.starts_with(prefix) {
        stem[prefix.len()..].to_string()
    } else {
        stem.to_string()
    }
}
```

### Step 5-3: `manager.rs` — ディレクトリ/パスロード

- [ ] `load_from_directory(dir)` を実装
  - `std::fs::read_dir(dir)` でエントリを列挙
  - `platform::lib_extension()` で拡張子フィルタ
  - 個別の `load_file` 失敗は `log::warn!` でスキップ
  - 成功した数を返す
- [ ] `load_from_directories(dirs)` を実装（各 dir に対して `load_from_directory` を呼ぶ）
- [ ] `load_paths(paths)` を実装
  - `path.is_dir()` → `load_from_directory`
  - `path.is_file()` → `load_file`
  - 存在しないパス → `log::warn!` でスキップ

### Step 5-4: `manager.rs` — get / names / plugins

- [ ] `get(name: &str) -> Option<&LoadedLibrary>` — `name_index` で index 取得 → `libraries[i]`
- [ ] `names() -> Vec<&str>` — `name_index.keys()` を収集
- [ ] `plugins() -> Vec<&LoadedLibrary>` — `libraries.iter().filter_map` で Some のものを収集

### Step 5-5: `manager.rs` — unload / unload_all / Drop

- [ ] `unload(name: &str) -> Result<(), PluginError>` を実装:
  1. `name_index.remove(name)` → None なら `PluginError::NotFound`
  2. `libraries[idx].take()` で `ManagedPlugin` を取得
  3. vtable が Some なら `(vtable.destroy)()` を呼ぶ
  4. `ManagedPlugin` を drop（`LoadedLibrary` が drop → `Library::close()`）
- [ ] `unload_all()` を実装:
  - `libraries` を逆順でイテレート
  - 各 `Some(managed)` に対して destroy → drop
  - `name_index.clear()`
- [ ] `Drop for PluginManager` — `self.unload_all()` を呼ぶ

- [ ] `lib.rs` に `pub mod manager;` と `pub use manager::PluginManager;` を追加

> **ファイル行数見積もり:** `manager.rs` は約 180-220 行。300 行を超えない。

### Step 5-6: Phase 5 テスト

`tests/integration.rs` に以下のテストを追加:

**正常系:**
- [ ] `test_manager_load_file_and_get` — `load_file` → `get("greeter")` → Some。`path()` が正しい
- [ ] `test_manager_names` — `load_file` 後に `names()` が `["greeter"]` を含む
- [ ] `test_manager_plugins` — `load_file` 後に `plugins()` が 1 つ返る
- [ ] `test_manager_unload` — `load_file` → `unload("greeter")` → `get("greeter")` = None
- [ ] `test_manager_load_from_directory` — ディレクトリスキャン → 少なくとも 1 つロード
- [ ] `test_manager_load_paths_mixed` — ファイルパスとディレクトリパスの混在
- [ ] `test_manager_drop_releases_all` — PluginManager を drop → 再度同じファイルを LoadedLibrary::load できる

**異常系:**
- [ ] `test_manager_duplicate_name` — 同じファイルを 2 回 `load_file` → 2 回目が `PluginError::DuplicateName`
- [ ] `test_manager_unload_nonexistent` — `unload("no_such")` → `PluginError::NotFound`
- [ ] `test_manager_load_from_nonexistent_directory` — 存在しないディレクトリ → `PluginError::Io`
- [ ] `test_manager_load_paths_nonexistent_skipped` — 存在しないパスが混在しても他は正常にロード

### Step 5-7: Phase 5 品質ゲート

- [ ] `cargo build -p dynplug-example && cargo test -p dynplug` で全テスト通過
- [ ] `cargo clippy -p dynplug -p dynplug-example -- -D warnings` が通る
- [ ] テストカバレッジが manager.rs で 90% 以上。未カバーの分岐があれば追加テストを書く
- [ ] `cargo build -p dynplug -p dynplug-example` が通る

---

## Phase 6: Example Host + 統合動作確認

### Step 6-1: `examples/host.rs` — ホスト側実行バイナリ

- [ ] `crates/dynplug/examples/host.rs` を作成。SoW Section 3 Phase 6 の完全なコードを実装する
  - Layer 1 (Symbol Bind): `plugin_entry` を bind して VTable 取得
  - Layer 2 (VTable): `vtable()` で greet, add, unknown method, panic_test を呼ぶ
  - PluginManager: `load_file` → `get` → `unload` → `load_from_directory`
  - 各ステップに assert を入れ、失敗時にメッセージを出力

### Step 6-2: 統合テスト総合確認

- [ ] `cargo build -p dynplug-example && cargo run -p dynplug --example host` を実行し、以下の出力が得られることを確認:
  - `=== dynplug Host Example ===`
  - Layer 1 / Layer 2 / PluginManager の各セクションで `assert` が全て通過
  - `=== All checks passed! ===`
- [ ] 失敗する場合は原因を特定し修正する（ロードパスの違い、マクロの展開ミス等）

### Step 6-3: Phase 6 品質ゲート

- [ ] `cargo build -p dynplug-example && cargo test -p dynplug` で全テスト通過（Phase 4 + 5 のテスト含む）
- [ ] `cargo build -p dynplug-example && cargo run -p dynplug --example host` が全 assert 通過
- [ ] `cargo clippy -p dynplug -p dynplug-example -- -D warnings` が通る
- [ ] `cargo fmt -p dynplug --check && cargo fmt -p dynplug-example --check` が通る
- [ ] テストカバレッジが全体（api.rs, error.rs, platform.rs, loader.rs, vtable.rs, export.rs, manager.rs）で 90% 以上。不足があれば追加テストを書く
- [ ] `cargo build -p dynplug -p dynplug-example` が通る

---

## Phase 7: Layer 3 — Safe Wrapper Macro (define_plugin!)

> **前提条件:** Phase 1-6 が全て完了し安定していること

### Step 7-1: `define.rs` — define_plugin! マクロ (VTable 生成)

- [ ] `crates/dynplug/src/define.rs` を作成
- [ ] `define_plugin!` マクロの VTable 生成部分を実装（RFC Section 5.8）
  - 入力: `pub struct Greeter { fn add(a: i32, b: i32) -> i32; ... }`
  - 出力: `#[repr(C)] pub struct GreeterVTable { interface_version: u32, add: extern "C" fn(i32, i32) -> i32, ... }`
  - FFI 型変換ルール（RFC Section 5.8 の型変換テーブル）:
    - プリミティブ (i32, u32, f64 等) → そのまま
    - `&str` 引数 → `*const u8, usize`
    - `String` 戻り値 → `*mut *mut u8, *mut usize` + return i32
    - `Result<T, PluginError>` → return i32 (0=ok, -1=err) + out params

### Step 7-2: `define.rs` — ホスト側ラッパー構造体生成

- [ ] `define_plugin!` マクロのラッパー構造体生成部分を実装
  - `Greeter` 構造体 — `_lib: LoadedLibrary` + `vtable: &'static GreeterVTable`
  - `Greeter::load(path)` — `LoadedLibrary::load` + `vtable::<GreeterVTable>(None)`
  - 各メソッド — FFI 型変換を行い vtable のフィールドを呼ぶ
  - `Drop for Greeter` — `(self.vtable.destroy)()`

### Step 7-3: `define.rs` — プラグイン側エクスポートマクロ生成

- [ ] `define_plugin!` が `export_{name}!` マクロも生成するようにする
  - `export_greeter!(add: my_add, greet: my_greet)` の形式
  - 各関数に `catch_unwind` ラッパーを生成
  - static VTable を生成
  - `#[no_mangle] pub extern "C" fn plugin_entry()` を生成

- [ ] `lib.rs` に `pub mod define;` を追加

> **ファイル行数見積もり:** `define.rs` は 200-350 行になる可能性がある。
> 350 行を超える場合は `define/mod.rs`, `define/vtable_gen.rs`, `define/wrapper_gen.rs` に分割する。

### Step 7-4: Phase 7 テスト

`tests/integration.rs` または `tests/layer3.rs` に以下のテストを追加:

**正常系:**
- [ ] `test_define_plugin_load` — `define_plugin!` で定義した Greeter 構造体で `Greeter::load(path)` が成功
- [ ] `test_define_plugin_primitive_call` — `greeter.add(21, 21)` が 42 を返す
- [ ] `test_define_plugin_string_call` — `greeter.greet("World")` が `Ok("Hello, World!")` を返す
- [ ] `test_define_plugin_drop` — Greeter を drop しても panic しない

**異常系:**
- [ ] `test_define_plugin_load_nonexistent` — `Greeter::load("/nonexistent")` が `PluginError::Load` を返す

### Step 7-5: Phase 7 品質ゲート

- [ ] `cargo build -p dynplug-example && cargo test -p dynplug` で全テスト通過（Phase 4-7 のテスト含む）
- [ ] `cargo clippy -p dynplug -p dynplug-example -- -D warnings` が通る
- [ ] `cargo fmt -p dynplug --check` が通る
- [ ] テストカバレッジが define.rs で 90% 以上。不足があれば追加テストを書く
- [ ] `cargo build -p dynplug -p dynplug-example` が通る

---

## Phase 8: 最終統合 — 動作確認 + ドキュメント

### Step 8-1: 動作確認 TODO リスト

以下の項目を順番に実行し、全て PASS するまで修正を繰り返す:

**ビルド確認:**
- [ ] `cargo build -p dynplug` が成功する
- [ ] `cargo build -p dynplug-example` が成功し、cdylib が生成される
- [ ] `cargo build -p dynplug --example host` が成功する
- [ ] `cargo clippy -p dynplug -p dynplug-example -- -D warnings` が通る
- [ ] `cargo fmt -p dynplug -p dynplug-example --check` が通る

**テスト確認:**
- [ ] `cargo build -p dynplug-example && cargo test -p dynplug` で全テスト通過
- [ ] テストカバレッジが全体で 90% 以上

**バイナリ動作確認:**
- [ ] `cargo build -p dynplug-example && cargo run -p dynplug --example host` を実行
- [ ] Layer 1 (Symbol Bind): plugin_entry をバインドし VTable のフィールドが呼べる → PASS / FAIL
- [ ] Layer 2 (VTable): vtable() で PluginVTable を取得できる → PASS / FAIL
- [ ] Layer 2 invoke greet: `"Hello, World!"` が返る → PASS / FAIL
- [ ] Layer 2 invoke add: `42` が返る → PASS / FAIL
- [ ] Layer 2 invoke unknown: rc=-1, エラーメッセージが取得できる → PASS / FAIL
- [ ] Layer 2 invoke panic: rc=-2, ホストがクラッシュしない → PASS / FAIL
- [ ] PluginManager load_file: ロード成功 → PASS / FAIL
- [ ] PluginManager get: 名前引き成功 → PASS / FAIL
- [ ] PluginManager unload: アンロード後に get が None → PASS / FAIL
- [ ] PluginManager load_from_directory: ディレクトリスキャン成功 → PASS / FAIL
- [ ] PluginManager Drop: 全プラグインが解放される → PASS / FAIL
- [ ] `=== All checks passed! ===` が出力される → PASS / FAIL

**エラーがある場合:** 原因を特定し修正。全項目が PASS するまで繰り返す。

### Step 8-2: README.md（英語）

- [ ] `crates/dynplug/README.md` を作成。以下の構成で記載する:

```markdown
# dynplug

> Cross-platform dynamic plugin loading for Rust

## Features
- 3-layer abstraction (Symbol Bind / VTable / Safe Wrapper macro)
- PluginManager for centralized lifecycle management
- Cross-platform: Linux, macOS, Windows, Android

## Installation
(cargo add or Cargo.toml dependency)

## Quick Start
### Host side (loading a plugin)
### Plugin side (creating a plugin)

## API Layers
### Layer 1: Symbol Bind
### Layer 2: VTable
### Layer 3: define_plugin! macro

## Building
(cargo build commands)

## Platform Support
(table)

## License
```

- [ ] 絵文字を適度に使用（セクション見出し程度。例: `## 🔌 Features`, `## 📦 Installation`）

### Step 8-3: README_ja.md（日本語版）

- [ ] `crates/dynplug/README_ja.md` を作成
- [ ] README.md と同じ構成で日本語に翻訳
- [ ] インストール手順、ビルド手順を日本語で記載

### Step 8-4: Phase 8 最終確認

- [ ] README.md のコード例が実際にコンパイル可能か確認
- [ ] `cargo build -p dynplug-example && cargo run -p dynplug --example host` が再度 PASS
- [ ] `cargo build -p dynplug-example && cargo test -p dynplug` が再度全テスト通過
- [ ] 全ファイルが 300 行以下であることを確認（超えている場合は分割する）
