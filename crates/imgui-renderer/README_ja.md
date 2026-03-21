# imgui-renderer

wgpu アプリケーション向けの Dear ImGui 統合ライブラリ。[imgui-rs](https://github.com/imgui-rs/imgui-rs) をベースに構築されています。

`ImGuiRenderer` は `imgui`、`imgui-wgpu`、`imgui-winit-support` を単一の型にラップし、**handle_event**、**frame**、**render** の3メソッドで構成されるシンプルな API を提供します。

## 特徴

- **ドッキング** — ImGui のドッキング機能がデフォルトで有効 (`ConfigFlags::DOCKING_ENABLE`)
- **オーバーレイモード** — `LoadOp::Load` で描画するため、既存のコンテンツの上に ImGui を重ねて表示
- **HiDPI 対応** — ウィンドウのスケールファクターに基づいてフォントサイズを自動調整
- **最小限の API** — 3つのコアメソッドで任意の wgpu + winit アプリケーションに統合可能
- **フルアクセス** — 内部の `imgui::Context` と `imgui_wgpu::Renderer` への直接アクセス
- **カスタムレンダーパス** — `render_into_pass()` で独自のレンダーパスに ImGui を埋め込み可能

## 要件

- **Rust** 1.70+
- **C++ コンパイラ** — imgui-rs が依存する [cimgui](https://github.com/cimgui/cimgui)（Dear ImGui の C バインディング）のビルドに必要
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Linux: `build-essential` または同等のパッケージ
  - Windows: MSVC (Visual Studio Build Tools)

## インストール

`Cargo.toml` に追加:

```toml
[dependencies]
imgui-renderer = { path = "crates/imgui-renderer" }
```

主要な依存関係（クレートが管理）:

| クレート | バージョン |
|---|---|
| `imgui` | 0.12 (`docking` feature 付き) |
| `imgui-wgpu` | 0.28 |
| `imgui-winit-support` | 0.13 |

## クイックスタート

```rust
use imgui_renderer::ImGuiRenderer;

// 1. レンダラーを作成（wgpu の device/queue と winit の window が準備できた後）
let mut imgui = ImGuiRenderer::new(
    &device,
    &queue,
    wgpu::TextureFormat::Bgra8UnormSrgb,
    &window,
).unwrap();

// 2. イベントハンドラで ImGui にイベントを転送
imgui.handle_event(&window, window_id, &event);

// 3. 各フレームで UI を構築して描画
imgui.frame(&window, |ui| {
    ui.window("Hello").build(|| {
        ui.text("Hello from ImGui!");
    });
});
imgui.render(&device, &queue, &texture_view);
```

## API リファレンス

### `ImGuiRenderer::new(device, queue, format, window) -> Result<Self>`

新しいレンダラーを作成します。ドッキング有効な ImGui コンテキストの初期化、HiDPI フォントスケーリングの設定、wgpu レンダラーのセットアップを行います。

### `handle_event(&mut self, window, window_id, event)`

`WindowEvent` を ImGui に転送して入力を処理します。`ApplicationHandler::window_event` 内ですべてのイベントに対して呼び出してください。

### `handle_non_window_event(&mut self, window, event)`

ウィンドウ以外のイベント（例: `Event::AboutToWait`）を ImGui に転送します。通常は `about_to_wait` から呼び出します。

### `frame(&mut self, window, f: FnOnce(&imgui::Ui))`

ImGui フレームを構築します。クロージャは `&imgui::Ui` を受け取り、UI を定義します。フレーム間のデルタタイムを自動的に計測します。

### `frame_with_dt(&mut self, window, dt: Duration, f: FnOnce(&imgui::Ui))`

`frame` と同じですが、自動計測の代わりに明示的なデルタタイムを使用します。

### `render(&mut self, device, queue, view)`

ImGui の描画データを指定されたテクスチャビューにレンダリングします。`LoadOp::Load` で独自のレンダーパスを作成します（オーバーレイモード）。

### `render_into_pass(&mut self, queue, device, rpass)`

既存のレンダーパスに ImGui の描画データをレンダリングします。レンダーパスの設定を完全に制御したい場合に使用します。

### アクセサ

| メソッド | 説明 |
|---|---|
| `context() -> &Context` | imgui `Context` への読み取り専用アクセス |
| `context_mut() -> &mut Context` | imgui `Context` への可変アクセス |
| `renderer() -> &Renderer` | imgui-wgpu `Renderer` への読み取り専用アクセス |
| `renderer_mut() -> &mut Renderer` | imgui-wgpu `Renderer` への可変アクセス |
| `reload_font_texture(device, queue)` | フォントの追加・削除後にフォントアトラスを再構築 |
| `want_capture_mouse() -> bool` | ImGui がマウス入力を要求しているか（独自のマウス処理をスキップすべきか） |
| `want_capture_keyboard() -> bool` | ImGui がキーボード入力を要求しているか（独自のキー処理をスキップすべきか） |

### 再エクスポート

コア依存クレートを便宜上再エクスポートしています:

```rust
use imgui_renderer::imgui;              // imgui-rs
use imgui_renderer::imgui_wgpu;         // imgui-wgpu レンダラー
use imgui_renderer::imgui_winit_support; // winit プラットフォームバックエンド
```

## サンプル

### スタンドアロンデモウィンドウ

ImGui デモウィンドウと FPS カウンターを表示するウィンドウを開きます。

```bash
cargo run -p imgui-renderer --example standalone
```

### オーバーレイ

色付き背景の上に ImGui を描画するデモです。背景色やパネルの透明度を操作できます。

```bash
cargo run -p imgui-renderer --example overlay
```

## 既存の wgpu アプリへの統合

既存の wgpu + winit アプリケーションに ImGui を追加する手順:

1. wgpu デバイスとウィンドウの初期化後に `ImGuiRenderer` を**作成**:

```rust
let mut imgui = ImGuiRenderer::new(&device, &queue, surface_format, &window)?;
```

2. `ApplicationHandler` で**イベントを転送**:

```rust
fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
    self.imgui.handle_event(&self.window, id, &event);

    // ImGui が入力を要求していない場合のみ独自の入力処理を実行
    if !self.imgui.want_capture_mouse() {
        // 独自のマウス入力処理
    }
}

fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    self.window.request_redraw();
    self.imgui.handle_non_window_event(&self.window, &Event::<()>::AboutToWait);
}
```

3. メインシーンの描画後に ImGui を**レンダリング**:

```rust
// シーンの描画（クリア、メッシュ描画など）
// ...

// ImGui をオーバーレイとして描画
imgui.frame(&window, |ui| {
    // UI の定義
});
imgui.render(&device, &queue, &view);

frame.present();
```

## ライセンス

親プロジェクト kalidokit-rust と同じライセンスです。
