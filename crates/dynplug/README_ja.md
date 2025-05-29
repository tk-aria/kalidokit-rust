# dynplug

> Rust 向けクロスプラットフォーム動的プラグインローディング

## 特徴

- **3 層の抽象化** -- ユースケースに応じて適切なレイヤーを選択:
  - **Layer 1 (Symbol Bind)**: 共有ライブラリから個別のシンボルをバインド
  - **Layer 2 (VTable)**: `#[repr(C)]` 関数ポインタテーブルによるバージョンチェック付き構造化アクセス
  - **Layer 3 (define_plugin!)**: トレイト風定義から VTable + 安全なラッパーを自動生成
- **PluginManager** -- ライフサイクルの一元管理とクリーンアップ保証
- **クロスプラットフォーム** -- Linux, macOS, Windows, Android
- **パニック安全** -- `export_plugin!` が FFI 境界でパニックを捕捉

## インストール

`Cargo.toml` に追加:

```toml
[dependencies]
dynplug = { version = "0.1", path = "crates/dynplug" }
```

## クイックスタート

### プラグインの作成 (cdylib)

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
dynplug = { version = "0.1", path = "../dynplug" }
```

```rust
// src/lib.rs
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
        _ => Err(format!("unknown method: {method}")),
    }
}
```

### プラグインの読み込み (ホスト側)

```rust
use dynplug::{LoadedLibrary, PluginManager};

// Layer 2: VTable
let lib = LoadedLibrary::load("path/to/plugin.dylib")?;
let vt = lib.vtable::<dynplug::PluginVTable>(None)?;

// Layer 3: define_plugin!
dynplug::define_plugin! {
    pub struct Calculator {
        fn add(a: i32, b: i32) -> i32;
    }
}
let calc = Calculator::load("path/to/calculator.dylib")?;
assert_eq!(calc.add(21, 21), 42);

// PluginManager
let mut manager = PluginManager::new();
manager.load_file("path/to/plugin.dylib")?;
manager.load_from_directory("plugins/")?;
let plugin = manager.get("greeter").unwrap();
```

## API レイヤー

### Layer 1: Symbol Bind

ライフタイム安全な個別シンボルバインド:

```rust
let lib = LoadedLibrary::load("libfoo.so")?;
let add = lib.bind::<extern "C" fn(i32, i32) -> i32>("add")?;
let result = add(21, 21); // Deref により自然に呼び出し可能
```

### Layer 2: VTable

バージョンチェック付き `#[repr(C)]` VTable の読み込み:

```rust
let lib = LoadedLibrary::load("plugin.dylib")?;
let vt = lib.vtable::<PluginVTable>(None)?;
// vt.invoke, vt.name, vt.version, vt.free_buffer, vt.destroy
```

標準の `PluginVTable` は汎用的な invoke インターフェースを提供します:

| 戻り値 | 意味 | `out_ptr` / `out_len` |
|:------:|------|----------------------|
| `0` | 成功 | 出力バッファ (使用後 `free_buffer` を呼ぶ) |
| `-1` | アプリケーションエラー | UTF-8 エラーメッセージ (`free_buffer` を呼ぶ) |
| `-2` | プラグインがパニック | 未定義 (`free_buffer` を呼ばない) |

### Layer 3: define_plugin!

VTable と安全なラッパーの自動生成 (v0.1: プリミティブ型のみ):

```rust
dynplug::define_plugin! {
    pub struct MyPlugin {
        fn compute(x: f64, y: f64) -> f64;
    }
}

// 以下が生成されます:
// - MyPluginVTable: 関数ポインタ + destroy を持つ #[repr(C)] 構造体
// - MyPlugin: load(path) と安全なメソッド呼び出しを持つラッパー
// - MyPluginVTable の VTableValidate 実装
// - destroy() を呼ぶ Drop 実装
```

### PluginManager

複数プラグインの一元的なライフサイクル管理:

```rust
let mut manager = PluginManager::new();

// ファイルやディレクトリから読み込み
manager.load_file("path/to/plugin.dylib")?;
manager.load_from_directory("plugins/")?;
manager.load_paths(["plugins/", "extra/libfoo.so"])?;

// 検索と一覧
let lib = manager.get("greeter").unwrap();
let names = manager.names();

// 個別または一括アンロード
manager.unload("greeter")?;
manager.unload_all(); // Drop 時にも呼ばれる (逆順)
```

## ビルド

```bash
# ライブラリのビルド
cargo build -p dynplug

# サンプルプラグインのビルド
cargo build -p dynplug-example

# ホストサンプルの実行
cargo build -p dynplug-example && cargo run -p dynplug --example host

# テストの実行
cargo build -p dynplug-example && cargo test -p dynplug

# lint
cargo clippy -p dynplug -- -D warnings
```

## プラットフォームサポート

| プラットフォーム | 拡張子 | 状態 |
|-----------------|--------|------|
| Linux x86_64 / aarch64 | `.so` | サポート済み |
| macOS x86_64 / aarch64 | `.dylib` | サポート済み |
| Windows x86_64 | `.dll` | サポート済み |
| Android aarch64 / armv7 | `.so` | サポート済み |
| iOS | -- | 非サポート (Apple ポリシー) |

## ライセンス

ワークスペースルートの LICENSE を参照してください。
