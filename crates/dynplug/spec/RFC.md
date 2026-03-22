# RFC: dynplug — Cross-Platform Dynamic Plugin Loading for Rust

- **Crate Name:** `dynplug`
- **Status:** Draft
- **Date:** 2026-03-23
- **Author:** tk-aria

---

## 1. Summary

Rust 用のクロスプラットフォーム動的プラグインローディングライブラリ。
共有ライブラリ（`.so` / `.dylib` / `.dll`）をランタイムで読み込み、
C ABI 経由で関数を呼び出すための型安全な抽象レイヤーを提供する。

## 2. Motivation

### 背景

- アプリケーションの機能を外部プラグインとして分離したい
- プラグインを再コンパイルなしに差し替え・追加可能にしたい
- Rust には安定した ABI がないため、動的ライブラリ境界では C ABI が必須
- `libloading` 単体では unsafe な FFI コードがユーザーに露出する

### 既存の選択肢と課題

| 既存 | 課題 |
|------|------|
| `libloading` 直接使用 | unsafe が散在、型安全性なし、ボイラープレート多い |
| `abi_stable` | 依存が大きい、プラグイン側も同クレートに依存必須 |
| `dlopen2` | `libloading` と大差なし |
| サブプロセス (go-plugin) | プロセス間通信のオーバーヘッド |

### dynplug が解決すること

- 3 段階の抽象レベルをユーザーが用途に応じて選択可能
- マクロでプラグイン側のボイラープレートを排除
- `PluginManager` による一元管理で解放忘れを防止
- クロスプラットフォーム（Linux, Windows, macOS, Android）

---

## 3. Glossary

| 用語 | 定義 |
|------|------|
| **ホスト** | プラグインをロードするアプリケーション |
| **プラグイン** | `cdylib` としてビルドされた共有ライブラリ |
| **VTable** | `#[repr(C)]` な関数ポインタの構造体。プラグインが `static` で保持 |
| **エントリーシンボル** | プラグインがエクスポートする `extern "C" fn() -> *const VTable` 関数 |
| **バインド** | ライブラリ内のシンボルを Rust の型付き関数として取得すること |

---

## 4. Layer Architecture

```text
Layer 3:  define_plugin! macro    greeter.add(21, 21)        最も安全・便利
Layer 2:  VTable                  (vtable.add)(21, 21)       構造化、型安全
Layer 1:  Symbol Bind             add(21, 21)                柔軟、アドホック
Layer 0:  LoadedLibrary           (内部基盤)                  直接使わない
```

**各レイヤーの使い分け:**

| ユースケース | 推奨レイヤー |
|------------|------------|
| 既存の C ライブラリ（自分で変更できない）を読み込みたい | Layer 1 |
| 自作プラグインで明確なインターフェースがある | Layer 2 |
| プラグインインターフェースの定義から FFI まで全て自動化したい | Layer 3 |

**レイヤー間の関係:**

- Layer 1 と Layer 2 は独立。同じライブラリに対して両方使える
- Layer 3 は内部で Layer 2 を使う（Layer 3 = Layer 2 のコード自動生成）
- プラグイン側: Layer 1 用には `#[no_mangle] pub extern "C" fn` を手書き。Layer 2/3 用には `export_plugin!` マクロを使う

---

## 5. Detailed Design

### 5.1 `api.rs` — C ABI 共通型

```rust
use std::os::raw::c_char;

/// ホスト側 dynplug とプラグイン側 dynplug のインターフェース互換性を示す。
/// PluginVTable の構造が変わった場合にインクリメントする。
/// ユーザー定義 VTable のバージョンとは別の概念。
pub const INTERFACE_VERSION: u32 = 1;

/// Layer 2 でプラグインがエクスポートするデフォルトのシンボル名。
/// `export_plugin!` マクロはこの名前で `#[no_mangle]` 関数を生成する。
/// ユーザー定義 VTable では別のシンボル名を使うことも可能（vtable() の引数で指定）。
pub const PLUGIN_ENTRY_SYMBOL: &str = "plugin_entry";

/// dynplug 標準の汎用 VTable。
///
/// `export_plugin!` マクロが自動生成する VTable の型。
/// ユーザーが `#[repr(C)]` で独自 VTable を定義する場合（Layer 2 直接使用）
/// にはこの型を使わなくてよい。
///
/// フィールドの並び順は ABI 互換性のため変更禁止。
/// 新しいフィールドは末尾に追加し、INTERFACE_VERSION をインクリメントする。
#[repr(C)]
pub struct PluginVTable {
    /// dynplug のインターフェースバージョン。INTERFACE_VERSION と一致すること。
    pub interface_version: u32,

    /// プラグイン名を返す。戻り値は null 終端 UTF-8 で、プラグインの static メモリを指す。
    /// ホスト側は戻り値を解放してはならない。
    pub name: extern "C" fn() -> *const c_char,

    /// プラグイン独自のバージョン番号を返す。
    pub version: extern "C" fn() -> u32,

    /// 汎用メソッド呼び出し。
    ///
    /// # 引数
    /// - method_ptr, method_len: UTF-8 メソッド名（null 終端不要）
    /// - input_ptr, input_len: 入力バイト列。input_len == 0 の場合 input_ptr は null 可
    /// - out_ptr: 成功時、プラグインが確保したバッファのポインタを書き込む
    /// - out_len: 成功時、出力バッファの長さを書き込む
    ///
    /// # 戻り値
    /// -  0: 成功。*out_ptr, *out_len に出力が書き込まれている。
    ///       ホスト側は使用後 free_buffer(*out_ptr, *out_len) を呼ぶこと。
    ///       出力が空の場合、*out_ptr = null, *out_len = 0 となり free_buffer 不要。
    /// - -1: プラグインが返したアプリケーションエラー。
    ///       *out_ptr, *out_len にエラーメッセージ (UTF-8) が書き込まれる。
    ///       ホスト側は読み取り後 free_buffer で解放すること。
    /// - -2: プラグイン内でパニックが発生した。out_ptr/out_len は未定義。
    pub invoke: extern "C" fn(
        method_ptr: *const u8,
        method_len: usize,
        input_ptr: *const u8,
        input_len: usize,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
    ) -> i32,

    /// invoke が *out_ptr に書き込んだバッファを解放する。
    /// プラグイン側のアロケータで確保されたメモリなので、
    /// ホスト側は直接 dealloc せず必ずこの関数を経由すること。
    /// ptr が null の場合は何もしない。
    pub free_buffer: extern "C" fn(ptr: *mut u8, len: usize),

    /// プラグインのリソースを解放する。
    /// LoadedLibrary::drop() (= Library::close()) の前に呼ばれる。
    /// 複数回呼ばれても安全であること（冪等性）。
    pub destroy: extern "C" fn(),
}

/// エントリーポイント関数の型。
/// プラグインは `#[no_mangle] pub extern "C" fn plugin_entry() -> *const PluginVTable`
/// としてエクスポートする。
pub type PluginEntryFn = extern "C" fn() -> *const PluginVTable;
```

### 5.2 `error.rs`

```rust
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// ライブラリファイルのロードに失敗
    #[error("failed to load library '{path}': {source}")]
    Load { path: String, source: String },

    /// 指定されたシンボルが見つからない
    #[error("symbol not found: '{symbol}' in '{path}'")]
    SymbolNotFound { symbol: String, path: String },

    /// PluginVTable.interface_version が INTERFACE_VERSION と一致しない
    #[error("interface version mismatch: host expects {host}, plugin has {plugin} (library: {path})")]
    VersionMismatch { host: u32, plugin: u32, path: String },

    /// エントリーシンボルが null を返した
    #[error("plugin entry returned null vtable (library: {path})")]
    NullVTable { path: String },

    /// PluginManager 内でプラグイン名が見つからない
    #[error("plugin not found: '{0}'")]
    NotFound(String),

    /// invoke が -1 を返した（プラグインのアプリケーションエラー）
    #[error("plugin invoke error: {message}")]
    Invoke { message: String },

    /// invoke 中にプラグインがパニックした（invoke が -2 を返した）
    #[error("plugin panicked during invoke")]
    Panic,

    /// PluginManager に同名プラグインが既にロードされている
    #[error("plugin '{0}' is already loaded")]
    DuplicateName(String),

    /// I/O エラー（ディレクトリスキャン等）
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

### 5.3 `platform.rs`

```rust
/// 共有ライブラリの拡張子（ドットなし）
pub fn lib_extension() -> &'static str {
    if cfg!(target_os = "windows") { "dll" }
    else if cfg!(target_os = "macos") { "dylib" }
    else { "so" }  // linux, android, freebsd, etc.
}

/// 共有ライブラリのファイル名プレフィックス
pub fn lib_prefix() -> &'static str {
    if cfg!(target_os = "windows") { "" }
    else { "lib" }
}

/// Cargo のクレート名からプラットフォーム固有のファイル名を生成する。
/// Cargo は crate name のハイフンをアンダースコアに変換するため、
/// この関数も同じ変換を行う。
///
/// 例: "dynplug-example" → "libdynplug_example.dylib" (macOS)
///                       → "libdynplug_example.so" (Linux)
///                       → "dynplug_example.dll" (Windows)
pub fn lib_filename(crate_name: &str) -> String {
    let name = crate_name.replace('-', "_");
    format!("{}{name}.{}", lib_prefix(), lib_extension())
}
```

### 5.4 Layer 0 + 1: `loader.rs` — LoadedLibrary + Symbol Bind

```rust
use std::path::{Path, PathBuf};

/// ロードされた共有ライブラリ。
///
/// Library のライフタイムを管理する。Drop 時に Library が close される。
/// VTable を使っている場合は、drop 前に VTable 経由で destroy() を呼ぶ必要がある。
/// (PluginManager を使う場合は自動で行われる。)
///
/// LoadedLibrary は Send + Sync であり、複数スレッドから同時に使用可能。
/// ただし、プラグイン側の関数がスレッドセーフかどうかはプラグイン実装者の責任。
pub struct LoadedLibrary {
    lib: libloading::Library,
    path: PathBuf,
}

impl LoadedLibrary {
    /// 指定パスの共有ライブラリをロードする。
    ///
    /// この時点ではシンボルの検索やバージョンチェックは行わない。
    /// 純粋に OS のローダー（dlopen / LoadLibrary）を呼ぶだけ。
    ///
    /// # Errors
    /// - `PluginError::Load`: ファイルが存在しない、権限がない、バイナリ形式が不正等
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PluginError> { ... }

    /// ロード元のファイルパスを返す。
    pub fn path(&self) -> &Path { &self.path }
}
```

**Symbol Bind (Layer 1):**

```rust
/// バインドされた関数ハンドル。
///
/// LoadedLibrary のライフタイムに束縛される。
/// Library が close された後にこのハンドルを使うと未定義動作になるため、
/// ライフタイムパラメータで防止する。
///
/// `Deref<Target = F>` を実装しており、関数ポインタとして直接呼び出せる。
///
/// # Example
/// ```ignore
/// let add = lib.bind::<extern "C" fn(i32, i32) -> i32>("add")?;
/// let result = add(21, 21);  // Deref で F に自動変換、普通の関数呼び出し
/// ```
pub struct BoundFn<'lib, F> {
    sym: libloading::Symbol<'lib, F>,
}

impl<F> std::ops::Deref for BoundFn<'_, F> {
    type Target = F;
    fn deref(&self) -> &F {
        &self.sym
    }
}

impl LoadedLibrary {
    /// ライブラリ内のシンボルを型付き関数としてバインドする。
    ///
    /// # 型パラメータ
    /// F は `extern "C" fn(...)` 型であること。呼び出し側が正しい型を指定する責任を持つ。
    /// 型が実際のシンボルと一致しない場合は未定義動作。
    ///
    /// # Errors
    /// - `PluginError::SymbolNotFound`: 指定名のシンボルが存在しない
    ///
    /// # Safety note
    /// この関数自体は safe だが、返された BoundFn の呼び出しは unsafe の意味的リスクがある
    /// （型パラメータの正しさはコンパイラが検証できないため）。
    /// 型安全性が必要な場合は Layer 2 (VTable) または Layer 3 (define_plugin!) を使うこと。
    pub fn bind<F>(&self, name: &str) -> Result<BoundFn<'_, F>, PluginError> { ... }
}
```

**Layer 1 でのプラグイン側の実装:**

Layer 1 はプラグイン側に dynplug への依存を要求しない。
プラグインは `#[no_mangle] pub extern "C" fn` を直接書くだけでよい。

```rust
// プラグイン側 (dynplug に依存しない)
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 { a + b }

#[no_mangle]
pub extern "C" fn greet(name_ptr: *const u8, name_len: usize) -> *const u8 { ... }
```

### 5.5 Layer 2: `vtable.rs` — VTable ロード

```rust
/// VTable 構造体が満たすべき契約。
///
/// # Safety
/// この trait を実装する型は以下を満たすこと:
/// 1. `#[repr(C)]` であること
/// 2. 先頭フィールドが `interface_version: u32` であること
/// 3. 全フィールドが `extern "C" fn` 型または C ABI 互換な型であること
///
/// これらの不変条件が破られると、vtable() 内の transmute が未定義動作を引き起こす。
pub unsafe trait VTableValidate {
    /// VTable 内の interface_version フィールドの値を返す。
    fn interface_version(&self) -> u32;
}

impl LoadedLibrary {
    /// エントリーシンボルから VTable を取得する。
    ///
    /// # 引数
    /// - entry_symbol: 省略可。None の場合は PLUGIN_ENTRY_SYMBOL ("plugin_entry") を使用。
    ///
    /// # 処理
    /// 1. entry_symbol で `extern "C" fn() -> *const V` をルックアップ
    /// 2. 関数を呼び出し、返された VTable ポインタが null でないことを確認
    /// 3. VTableValidate::interface_version() と INTERFACE_VERSION を比較
    /// 4. 一致すれば &'static V として返す
    ///
    /// # ライフタイム
    /// 戻り値は 'static だが、実際にはプラグインの static メモリを参照している。
    /// LoadedLibrary が drop されると（Library が close されると）ダングリング参照になる。
    /// → PluginManager はこの問題を、destroy → close の順序制御で防止する。
    /// → LoadedLibrary を直接使う場合はユーザーの責任。
    ///
    /// # Errors
    /// - `PluginError::SymbolNotFound`: エントリーシンボルが存在しない
    /// - `PluginError::NullVTable`: エントリー関数が null を返した
    /// - `PluginError::VersionMismatch`: interface_version 不一致
    pub fn vtable<V: VTableValidate>(
        &self,
        entry_symbol: Option<&str>,
    ) -> Result<&'static V, PluginError> { ... }
}
```

**Layer 2 でのプラグイン側:**

```rust
// GreeterVTable はホスト・プラグインで共有する共通クレートで定義するか、
// 同じ定義を双方にコピーする

#[repr(C)]
pub struct GreeterVTable {
    pub interface_version: u32,
    pub add: extern "C" fn(i32, i32) -> i32,
    pub version: extern "C" fn() -> u32,
    pub destroy: extern "C" fn(),
}

static VTABLE: GreeterVTable = GreeterVTable {
    interface_version: 1,
    add: my_add,
    version: my_version,
    destroy: my_destroy,
};

#[no_mangle]
pub extern "C" fn plugin_entry() -> *const GreeterVTable {
    &VTABLE
}

extern "C" fn my_add(a: i32, b: i32) -> i32 { a + b }
extern "C" fn my_version() -> u32 { 1 }
extern "C" fn my_destroy() {}
```

### 5.6 `export.rs` — export_plugin! マクロ（Layer 2 の PluginVTable 用）

`export_plugin!` は `api.rs` の **汎用 `PluginVTable`** を自動生成する。
ユーザー定義 VTable（Layer 2 直接）を使う場合はこのマクロは不要。

```rust
/// # 展開例
///
/// 入力:
/// ```ignore
/// dynplug::export_plugin! {
///     name: "greeter",
///     version: 1,
///     invoke: handle_invoke,
/// }
/// ```
///
/// 展開後（概念的。実際のマクロ衛生によりシンボル名は異なる場合がある）:
///
/// ```ignore
/// extern "C" fn __dynplug_name() -> *const std::os::raw::c_char {
///     // "greeter\0" の static バイトへのポインタ
///     b"greeter\0".as_ptr() as *const std::os::raw::c_char
/// }
///
/// extern "C" fn __dynplug_version() -> u32 {
///     1
/// }
///
/// extern "C" fn __dynplug_invoke(
///     method_ptr: *const u8, method_len: usize,
///     input_ptr: *const u8, input_len: usize,
///     out_ptr: *mut *mut u8, out_len: *mut usize,
/// ) -> i32 {
///     // 1. catch_unwind でパニック境界を設置
///     // 2. method_ptr/method_len → &str に変換
///     // 3. input_ptr/input_len → &[u8] に変換 (input_len == 0 なら &[])
///     // 4. ユーザーの handle_invoke(method, input) を呼ぶ
///     // 5. Ok(buf):
///     //      buf が空なら *out_ptr = null, *out_len = 0, return 0
///     //      buf が非空なら Box::into_raw(buf.into_boxed_slice()) で
///     //      *out_ptr, *out_len に書き込み、return 0
///     // 6. Err(msg):
///     //      msg を UTF-8 バイト列として同様に *out_ptr, *out_len に書き込み、return -1
///     // 7. catch_unwind が Err (パニック):
///     //      return -2。out_ptr/out_len は書き込まない
/// }
///
/// extern "C" fn __dynplug_free_buffer(ptr: *mut u8, len: usize) {
///     if !ptr.is_null() && len > 0 {
///         unsafe {
///             // Box::into_raw で確保したメモリを Box::from_raw で回収
///             drop(Box::from_raw(std::slice::from_raw_parts_mut(ptr, len)));
///         }
///     }
/// }
///
/// extern "C" fn __dynplug_destroy() {
///     // デフォルト実装: 何もしない
///     // プラグインがグローバルリソースを持つ場合はカスタム実装が必要
/// }
///
/// static __DYNPLUG_VTABLE: dynplug::PluginVTable = dynplug::PluginVTable {
///     interface_version: dynplug::INTERFACE_VERSION,
///     name: __dynplug_name,
///     version: __dynplug_version,
///     invoke: __dynplug_invoke,
///     free_buffer: __dynplug_free_buffer,
///     destroy: __dynplug_destroy,
/// };
///
/// #[no_mangle]
/// pub extern "C" fn plugin_entry() -> *const dynplug::PluginVTable {
///     &__DYNPLUG_VTABLE
/// }
/// ```
```

### 5.7 `manager.rs` — PluginManager

```rust
/// 複数プラグインのライフサイクルを一元管理する。
///
/// # 所有権と解放順序
///
/// PluginManager は全 LoadedLibrary の唯一の所有者。
/// Drop 時の解放順序:
/// 1. 各プラグインの VTable::destroy() を呼ぶ（VTable がロード済みの場合）
/// 2. Library::close() が呼ばれる（LoadedLibrary の Drop）
///
/// # 名前の一意性
///
/// プラグイン名は PluginManager 内で一意でなければならない。
/// 同名のプラグインをロードしようとすると PluginError::DuplicateName を返す。
///
/// # スレッド安全性
///
/// PluginManager 自体は !Sync。単一スレッドから操作すること。
/// ロード済みの LoadedLibrary を &参照で複数スレッドから使うのは可能
/// （プラグイン側関数のスレッド安全性はプラグイン実装者の責任）。
pub struct PluginManager {
    /// name → index のマップ（名前引き用）
    name_index: HashMap<String, usize>,
    /// ロード済みライブラリ。unload 時は None に置き換え（Vec の index を安定に保つため）
    libraries: Vec<Option<ManagedPlugin>>,
}

/// PluginManager が内部で管理するプラグイン情報
struct ManagedPlugin {
    library: LoadedLibrary,
    /// PluginVTable がロード済みなら保持（destroy 呼び出し用）
    vtable: Option<&'static PluginVTable>,
    /// プラグイン名（PluginVTable::name() から取得、または vtable 未ロードなら
    /// ファイル名から推測）
    name: String,
}

impl PluginManager {
    /// 空の PluginManager を生成する。
    pub fn new() -> Self;

    /// 単一ファイルをロードする。
    ///
    /// プラグイン名の取得:
    /// 1. PLUGIN_ENTRY_SYMBOL でエントリーを試行
    /// 2. 成功すれば VTable::name() からプラグイン名を取得
    /// 3. 失敗すれば（Layer 1 専用プラグイン）ファイルステムをプラグイン名とする
    ///    例: "libgreeter.dylib" → "greeter"（lib プレフィックスと拡張子を除去）
    ///
    /// # Errors
    /// - `PluginError::Load`: ロード失敗
    /// - `PluginError::DuplicateName`: 同名プラグインが既にロード済み
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<&LoadedLibrary, PluginError>;

    /// ディレクトリ内の共有ライブラリを全てロードする。
    ///
    /// プラットフォーム固有の拡張子（.so/.dylib/.dll）を持つファイルのみ対象。
    /// サブディレクトリは探索しない（非再帰）。
    /// 個別のロード失敗は log::warn で報告し、スキップして続行する。
    ///
    /// # Returns
    /// 成功したロード数
    pub fn load_from_directory(&mut self, dir: impl AsRef<Path>) -> Result<usize, PluginError>;

    /// 複数ディレクトリをスキャンする。
    pub fn load_from_directories(
        &mut self,
        dirs: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Result<usize, PluginError>;

    /// ファイルパスとディレクトリパスを混在させて一括ロード。
    ///
    /// 各パスに対して:
    /// - ディレクトリなら load_from_directory
    /// - ファイルなら load_file
    /// - 存在しないパスは log::warn でスキップ
    pub fn load_paths(
        &mut self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Result<usize, PluginError>;

    /// プラグイン名で LoadedLibrary を取得する。
    pub fn get(&self, name: &str) -> Option<&LoadedLibrary>;

    /// ロード済み全プラグインの名前一覧を返す。
    pub fn names(&self) -> Vec<&str>;

    /// ロード済み全プラグインを返す。
    pub fn plugins(&self) -> Vec<&LoadedLibrary>;

    /// 指定プラグインをアンロードする。
    ///
    /// 処理順序:
    /// 1. VTable がロード済みなら destroy() を呼ぶ
    /// 2. LoadedLibrary を drop（Library::close()）
    /// 3. name_index から除去
    ///
    /// # Errors
    /// - `PluginError::NotFound`: 指定名のプラグインがロードされていない
    ///
    /// # Warning
    /// この時点でユーザーが bind() や vtable() で取得した参照を保持している場合、
    /// ダングリング参照になる。ユーザーの責任で先に参照を破棄すること。
    pub fn unload(&mut self, name: &str) -> Result<(), PluginError>;

    /// 全プラグインをアンロードする。
    /// ロード順の逆順で destroy → close を行う。
    pub fn unload_all(&mut self);
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.unload_all();
    }
}
```

### 5.8 Layer 3: `define.rs` — Safe Wrapper マクロ

> **注意:** Layer 3 は Phase 7（最終フェーズ）で実装する。Phase 1-6 が安定した後に着手。
> 宣言マクロ（`macro_rules!`）で実装する。対応しきれない場合は proc macro に移行する
> （その場合は `dynplug-macros` クレートを追加）。

```rust
/// VTable + 安全なラッパー構造体 + プラグイン側エクスポートマクロを自動生成する。
///
/// # 入力構文
/// ```ignore
/// dynplug::define_plugin! {
///     /// ドキュメントコメント（省略可）
///     pub struct Greeter {
///         fn add(a: i32, b: i32) -> i32;
///         fn greet(name: &str) -> Result<String, PluginError>;
///     }
/// }
/// ```
///
/// # 生成されるもの
///
/// ## 1. VTable 構造体
/// ```ignore
/// #[repr(C)]
/// pub struct GreeterVTable {
///     pub interface_version: u32,
///     pub add: extern "C" fn(i32, i32) -> i32,
///     pub greet: extern "C" fn(*const u8, usize, *mut *mut u8, *mut usize) -> i32,
///     pub free_buffer: extern "C" fn(*mut u8, usize),
///     pub destroy: extern "C" fn(),
/// }
///
/// unsafe impl dynplug::VTableValidate for GreeterVTable {
///     fn interface_version(&self) -> u32 { self.interface_version }
/// }
/// ```
///
/// ## 2. ホスト側ラッパー構造体
/// ```ignore
/// pub struct Greeter {
///     _lib: dynplug::LoadedLibrary,
///     vtable: &'static GreeterVTable,
/// }
///
/// impl Greeter {
///     pub fn load(path: impl AsRef<Path>) -> Result<Self, dynplug::PluginError> {
///         let lib = dynplug::LoadedLibrary::load(path)?;
///         let vtable = lib.vtable::<GreeterVTable>(None)?;
///         Ok(Self { _lib: lib, vtable })
///     }
///
///     pub fn add(&self, a: i32, b: i32) -> i32 {
///         (self.vtable.add)(a, b)
///     }
///
///     pub fn greet(&self, name: &str) -> Result<String, dynplug::PluginError> {
///         // ptr+len に変換 → FFI 呼び出し → 戻り値変換 → free_buffer
///     }
/// }
///
/// impl Drop for Greeter {
///     fn drop(&mut self) { (self.vtable.destroy)(); }
/// }
/// ```
///
/// ## 3. プラグイン側エクスポートマクロ
/// ```ignore
/// macro_rules! export_greeter {
///     (add: $add:ident, greet: $greet:ident $(,)?) => {
///         // catch_unwind + FFI ブリッジ関数を生成
///         // static GreeterVTable を生成
///         // #[no_mangle] pub extern "C" fn plugin_entry() → &GreeterVTable
///     };
/// }
/// ```
///
/// # FFI 型変換ルール
///
/// | Rust 引数型 | C ABI | 変換 |
/// |------------|-------|------|
/// | i32, u32, i64, u64, f32, f64, bool | そのまま | なし |
/// | &str | *const u8, usize | from_raw_parts → from_utf8_unchecked |
/// | &[u8] | *const u8, usize | from_raw_parts |
///
/// | Rust 戻り値型 | C ABI | 変換 |
/// |-------------|-------|------|
/// | i32, u32, i64, u64, f32, f64, bool | そのまま | なし |
/// | String | out_ptr + out_len + return i32 | Box::into_raw(into_boxed_slice()) |
/// | Vec<u8> | out_ptr + out_len + return i32 | Box::into_raw(into_boxed_slice()) |
/// | Result<T, PluginError> | return i32 (0=ok, -1=err) + out params | 上記 + エラーメッセージ |
```

---

## 6. Safety Invariants（安全性不変条件）

### 6.1 メモリ所有権ルール

```text
プラグインが確保                ホストが使用                  プラグインが解放
    │                            │                            │
    ▼                            ▼                            ▼
 invoke() で                 *out_ptr を読む              free_buffer(ptr, len)
 Box::into_raw()                                          Box::from_raw()
```

- **ルール 1:** ホスト側はプラグインが返したポインタを直接 dealloc してはならない
- **ルール 2:** invoke が 0 を返し、`*out_len > 0` の場合、ホストは `free_buffer` を呼ぶ義務がある
- **ルール 3:** invoke が -1 を返した場合も、`*out_ptr` にエラー文字列が入る。ホストは `free_buffer` で解放する
- **ルール 4:** invoke が -2 を返した場合、`*out_ptr`/`*out_len` は未定義。`free_buffer` を呼んではならない

### 6.2 ライフタイムルール

```text
LoadedLibrary の生存期間
|<──────────────────────────────────────>|
   BoundFn の生存期間
   |<─────────────────>|  ← ライフタイムで強制 ('lib)

   vtable 参照の生存期間
   |<─────────────────────────>|  ← 'static だがダングリングに注意
                                  PluginManager は destroy → close の順序で防止
```

- `BoundFn<'lib, F>` はライフタイムパラメータで Library の生存と結合（コンパイラが強制）
- `&'static VTable` は Library の close 後にダングリングになる可能性がある
  - `PluginManager` を使えば安全（Drop で正しい順序で解放）
  - `LoadedLibrary` を直接使う場合はユーザーの責任

### 6.3 スレッド安全性

- `LoadedLibrary` は `Send + Sync`
- `BoundFn` は `Send + Sync`（内部の libloading::Symbol が Send + Sync のため）
- プラグイン側の関数がスレッドセーフかはプラグイン実装者の責任
- dynplug はスレッド安全性を保証しない。ドキュメントで注意喚起する

### 6.4 パニック境界

- `export_plugin!` マクロが生成する invoke ブリッジは `catch_unwind` でラップする
- ユーザー定義 VTable（Layer 2 直接）ではパニック境界はプラグイン実装者の責任
- FFI 境界を越えるパニックは未定義動作であることをドキュメントに明記

---

## 7. Platform Support

| Platform | Extension | Prefix | Load | Status |
|----------|-----------|--------|------|--------|
| Linux x86_64/aarch64 | `.so` | `lib` | `dlopen` | Supported |
| macOS x86_64/aarch64 | `.dylib` | `lib` | `dlopen` | Supported |
| Windows x86_64 | `.dll` | (none) | `LoadLibrary` | Supported |
| Android aarch64/armv7 | `.so` | `lib` | `dlopen` | Supported |
| iOS | — | — | — | **Unsupported** (Apple policy) |

iOS: Apple の App Store ポリシーにより実行時コードロードが禁止。
ローダー実装には `#[cfg(target_os)]` 分岐が存在しないため、
将来 `dlopen` が解禁された場合は `platform.rs` の拡張子テーブルに `"dylib"` を返す分岐を追加するだけで対応可能。

---

## 8. Alternatives Considered

| 代替案 | 不採用理由 |
|--------|-----------|
| WASM (wasmtime/wasmer) | パフォーマンスオーバーヘッド、ホスト API 公開の複雑さ。別クレートとして将来検討 |
| `abi_stable` | 依存が大きく、プラグイン側も同クレートに依存必須。C ライブラリ非対応 |
| iOS 静的リンク (`linkme`) | 対象外とした。Apple が dlopen を解禁した際にすぐ使えることを優先 |
| proc macro (`#[dynplug::interface]`) | v0.1 では `macro_rules!` で実装。不十分なら v0.2 で proc macro に移行 |

---

## 9. Unresolved Questions

| # | 問題 | 影響範囲 | 決定期限 |
|---|------|---------|---------|
| 1 | `define_plugin!` を `macro_rules!` で実現できるか | Phase 7 | Phase 6 完了後 |
| 2 | Android NDK のライブラリパス解決ヘルパーの具体的 API | `platform.rs` | Android テスト時 |
| 3 | ホットリロード（ファイル監視）を dynplug に含めるか | `manager.rs` | v0.2 以降 |

---

## 10. References

- [libloading docs](https://docs.rs/libloading) — Cross-platform dynamic library loading
- [abi_stable docs](https://docs.rs/abi_stable) — Stable ABI for Rust
- [Rust FFI Omnibus](http://jakegoulding.com/rust-ffi-omnibus/) — FFI patterns reference
- [HashiCorp go-plugin](https://github.com/hashicorp/go-plugin) — Subprocess-based plugin system
