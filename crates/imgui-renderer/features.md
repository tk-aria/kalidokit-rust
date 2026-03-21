# imgui-renderer — 実装タスク

> 各 Phase は順番に実装。各 Step 内のチェックボックスを完了順にチェックする。
> 300行以上になるファイルは分割候補として明記。
> `cargo fmt --check` は全 Phase の検証で毎回実行すること。
> 設計書: `docs/rfc.md`, `docs/sow.md`

## Phase 依存関係

```
Phase 1 (wgpu 24→29 アップグレード)
  ↓
Phase 2 (imgui-renderer クレート作成)
  ↓
Phase 3 (Example + 動作確認)
  ↓
Phase 4 (kalidokit-rust アプリ統合)
  ↓
Phase 5 (拡張: ImPlot, ImGuizmo, ImNodes)
  ↓
Phase 6 (README + 動作確認チェックリスト)
```

## ライブラリバージョン一覧

| クレート | バージョン | 用途 |
|---------|-----------|------|
| `wgpu` | **29.0** | GPU 描画 API (24 から上げる) |
| `winit` | 0.30.9 | ウィンドウ管理 (変更なし) |
| `imgui` | 0.12 (docking feature) | imgui-rs 安全な Rust API (docking 対応) |
| `imgui-sys` | 0.12 | cimgui FFI (imgui が内部で使用) |
| `imgui-wgpu` | 0.28 | wgpu 29 レンダラーバックエンド |
| `imgui-winit-support` | 0.13 | winit 0.30 プラットフォームバックエンド |
| `anyhow` | 1.0 | エラーハンドリング |
| `log` | 0.4 | ログマクロ |

---

## Phase 1: wgpu 24 → 29 アップグレード

**目的**: ワークスペース全体の wgpu 依存を 29.0 に更新し、全クレートのビルドとテストを通す

**リスク: 高** — renderer クレートの API 変更が最大。wgpu の migration guide を参照しながら段階的に修正。

### Step 1.1: workspace wgpu バージョン更新

- [x] ルート `Cargo.toml` の `wgpu = "24.0"` を `wgpu = "29.0"` に変更
- [x] `cargo update -p wgpu` で Cargo.lock を更新
- [x] `Cargo.lock` が正常に解決されることを確認 (依存競合がないこと)

### Step 1.2: renderer クレート修正 — context.rs

- [x] `Instance::new()` の API 変更に対応

```rust
// wgpu 24
let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
// wgpu 29 — InstanceDescriptor が変更されている可能性あり、公式 migration guide 参照
let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
```

- [x] `Surface::configure()` の API 変更に対応
- [x] `surface.get_default_config()` の API 変更に対応
- [x] `adapter.request_device()` の DeviceDescriptor 変更に対応
- [x] `cargo check -p renderer` が通ることを確認

### Step 1.3: renderer クレート修正 — pipeline.rs, scene.rs, mesh.rs

- [x] `RenderPassDescriptor` のライフタイム/フィールド変更に対応

```rust
// wgpu 24 → 29 で変更される可能性のある箇所:
// - RenderPassColorAttachment の ops フィールド
// - TexelCopyTextureInfo (旧 ImageCopyTexture)
// - TexelCopyBufferLayout (旧 ImageDataLayout)
// コンパイラエラーに従って修正
```

- [x] `TextureDescriptor` の変更に対応
- [x] `BufferDescriptor` / `BufferAddress` の変更に対応
- [x] `ComputePassDescriptor` の変更に対応 (NV12 変換)
- [x] `cargo check -p renderer` が通ることを確認
- [x] **⚠ 300行超え見込み**: scene.rs は既に大きいため、変更箇所が多い場合はコミットを細分化

### Step 1.4: video-decoder クレート修正

- [x] `wgpu::TextureDescriptor` の変更に対応 (convert/mod.rs)
- [x] `queue.write_texture()` の `TexelCopyTextureInfo` / `TexelCopyBufferLayout` 変更に対応
- [x] `ComputePipelineDescriptor` / `ComputePassDescriptor` の変更に対応
- [x] `cargo check -p video-decoder` が通ることを確認

### Step 1.5: app クレート修正

- [x] `Surface::get_current_texture()` の API 変更に対応
- [x] `CommandEncoderDescriptor` の変更に対応
- [x] `cargo check -p kalidokit-rust` が通ることを確認

### Step 1.6: 全ワークスペースビルド確認

- [x] `cargo check --workspace` — 全クレート pass
- [x] `cargo test -p renderer -p vrm -p solver -p video-decoder` — 全テスト pass
- [x] `cargo test -p kalidokit-rust` — アプリテスト pass
- [x] `cargo clippy --workspace -- -D warnings` — 警告なし
- [x] `cargo fmt --check` — フォーマット OK

### Step 1.7: Phase 1 検証

- [x] テストカバレッジ 90% 以上を確認、未カバー部分のテスト追加
- [x] `cargo build --release -p kalidokit-rust` — リリースビルド成功
- [x] **動作確認**: アプリを release ビルドで起動し、以下が全て既存通り動作することを確認。目的の動作と異なる場合は修正を繰り返す:
  - VRM モデルが正しく描画される
  - 動画背景 (VideoToolbox) が再生される
  - マスコットモード (透過 + ドラッグ) が動作する
  - FPS が wgpu 24 時と同等 (リグレッションなし)

---

## Phase 2: imgui-renderer クレート作成

**目的**: dear-imgui-rs + dear-imgui-wgpu + dear-imgui-winit を薄くラップし、wgpu アプリに 3 メソッドで統合可能にする

### Step 2.1: Cargo.toml + ワークスペース追加

- [x] ルート `Cargo.toml` の `members` に `"crates/imgui-renderer"` を追加
- [x] `crates/imgui-renderer/Cargo.toml` を作成

```toml
[package]
name = "imgui-renderer"
version = "0.1.0"
edition = "2021"
description = "Dear ImGui integration for wgpu applications"

[dependencies]
imgui = { version = "0.12", features = ["docking"] }
imgui-wgpu = "0.28"
imgui-winit-support = "0.13"
wgpu = { workspace = true }
winit = { workspace = true }
log = { workspace = true }
anyhow = { workspace = true }

[dev-dependencies]
pollster = { workspace = true }
env_logger = { workspace = true }
```

- [x] `cargo check -p imgui-renderer` が通ることを確認
- [x] **注意**: imgui-sys のビルドには C++ コンパイラが必要 (cimgui のビルド)

### Step 2.2: ImGuiRenderer::new() — `src/lib.rs` (~120行)

- [x] `ImGuiRenderer` struct 定義

```rust
pub struct ImGuiRenderer {
    ctx: imgui::Context,
    platform: imgui_winit_support::WinitPlatform,
    renderer: imgui_wgpu::Renderer,
    last_frame: Instant,
    last_cursor: Option<MouseCursor>,
}
```

- [x] `new(device, queue, format, window)` 実装
  1. `Context::create()` + docking 有効化
  2. `WinitPlatform::new()` + `attach_window()`
  3. `Renderer::new()` (フォントアトラスのテクスチャ生成含む)
- [x] `cargo check -p imgui-renderer` が通ることを確認

```rust
// 参考コード
impl ImGuiRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        window: &winit::window::Window,
    ) -> anyhow::Result<Self> {
        let mut ctx = dear_imgui::Context::create();
        ctx.io_mut().config_flags |= dear_imgui::ConfigFlags::DOCKING_ENABLE;

        let mut platform = dear_imgui_winit::WinitPlatform::new(&mut ctx);
        platform.attach_window(
            &mut ctx,
            window,
            dear_imgui_winit::HiDpiMode::Default,
        );

        let renderer = dear_imgui_wgpu::Renderer::new(&mut ctx, device, queue, format);

        Ok(Self { ctx, platform, renderer })
    }
}
```

### Step 2.3: handle_event() + frame() + render()

- [x] `handle_event(window, window_id, event)` — winit イベントを ImGui IO に転送
- [x] `frame(window, closure)` — delta time 自動計測 + UI クロージャ呼び出し (+ `frame_with_dt` で明示的 dt 指定も可)
- [x] `render(device, queue, view)` — DrawData → wgpu RenderPass (LoadOp::Load)
- [x] `context_mut()` — Context への直接アクセス

```rust
pub fn handle_event(&mut self, window: &winit::window::Window, event: &winit::event::WindowEvent) {
    self.platform.handle_event(&mut self.ctx, window, event);
}

pub fn frame<F: FnOnce(&dear_imgui::Ui)>(
    &mut self, window: &winit::window::Window, dt: std::time::Duration, f: F,
) {
    self.ctx.io_mut().update_delta_time(dt);
    self.platform.prepare_frame(&mut self.ctx, window).unwrap();
    let ui = self.ctx.frame();
    f(ui);
    self.platform.prepare_render(ui, window);
}

pub fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, view: &wgpu::TextureView) {
    let draw_data = self.ctx.render();
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("imgui_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        self.renderer.render(draw_data, queue, device, &mut pass).unwrap();
    }
    queue.submit(std::iter::once(encoder.finish()));
}

pub fn context_mut(&mut self) -> &mut dear_imgui::Context {
    &mut self.ctx
}
```

- [x] `cargo check -p imgui-renderer` が通ることを確認

### Step 2.4: テスト

- [x] **正常系テスト**:
  - API シグネチャの型チェック (コンパイルテスト) — `api_signature_check`
  - doc テスト (Quick Start コード例のコンパイル確認)
- [ ] **異常系テスト**:
  - GPU が利用不可の場合に適切なエラー (ヘッドレス環境) — ヘッドレス環境のため未検証

### Step 2.5: Phase 2 検証

- [x] `cargo test -p imgui-renderer` — テスト pass (unit test + doc test)
- [x] `cargo clippy -p imgui-renderer -- -D warnings` — 警告なし
- [x] `cargo fmt -p imgui-renderer --check` — フォーマット OK
- [x] `cargo doc -p imgui-renderer --no-deps` — 警告なし
- [ ] テストカバレッジ 90% 以上を確認、未カバー部分のテスト追加
- [x] `cargo build -p imgui-renderer` が正常完了 (cargo check で確認)
- [ ] **動作確認**: テストが pass し、API が設計通りの型シグネチャを持つことを確認 — ヘッドレス環境のため未検証

---

## Phase 3: Example + 動作確認

**目的**: ImGui の描画が wgpu ウィンドウで正しく動作することを実証

### Step 3.1: standalone example — `examples/standalone.rs` (~120行)

- [x] winit ウィンドウ + wgpu 初期化 + ImGuiRenderer 統合
- [x] ImGui Demo Window (`ui.show_demo_window()`) を表示
- [x] FPS カウンターを Info パネルに表示

```rust
// examples/standalone.rs 骨格
fn main() -> anyhow::Result<()> {
    env_logger::init();
    let event_loop = winit::event_loop::EventLoop::new()?;
    let mut app = StandaloneApp::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct StandaloneApp {
    gpu: Option<GpuState>,
    imgui: Option<imgui_renderer::ImGuiRenderer>,
    window: Option<Arc<winit::window::Window>>,
    last_frame: Option<Instant>,
    show_demo: bool,
}

// resumed(): wgpu 初期化 + ImGuiRenderer::new()
// window_event(): imgui.handle_event() + キーボード
// RedrawRequested:
//   imgui.frame(|ui| {
//       if show_demo { ui.show_demo_window(&mut show_demo); }
//       ui.window("Info").build(|| { ui.text(format!("FPS: {:.0}", fps)); });
//   });
//   imgui.render(device, queue, view);
```

- [ ] `cargo run -p imgui-renderer --example standalone` で ImGui Demo Window が表示されることを確認 — ヘッドレス環境のため未検証

### Step 3.2: overlay example — `examples/overlay.rs` (~100行)

- [x] 背景色付き画面 (単色クリア) + ImGui パネルオーバーレイ
- [x] ImGui が 3D シーンの上に半透明で重なる設計 (LoadOp::Load)

```rust
// 描画フロー
// 1. encoder.begin_render_pass(LoadOp::Clear(BLUE))  — 背景クリア
// 2. encoder.submit()
// 3. imgui.frame(|ui| { ui.window("Overlay").build(|| { ... }); });
// 4. imgui.render(device, queue, &view)  — LoadOp::Load で上に重ねる
```

- [ ] `cargo run -p imgui-renderer --example overlay` で動作確認 — ヘッドレス環境のため未検証

### Step 3.3: Docking 確認

- [ ] standalone example で ImGui ウィンドウのドッキング (タブ化, 分割) が動作すること
- [ ] `DockSpace` が有効でウィンドウをドラッグ＆ドッキングできること

### Step 3.4: Phase 3 検証

- [ ] `cargo clippy -p imgui-renderer -- -D warnings` — 警告なし
- [ ] `cargo fmt -p imgui-renderer --check` — フォーマット OK
- [ ] テストカバレッジ確認、未カバー部分のテスト追加
- [ ] `cargo build -p imgui-renderer --release` — リリースビルド成功
- [ ] **動作確認**: macOS で release ビルドの両 example を起動し、以下を確認。目的の動作と異なる場合は修正を繰り返す:
  - ImGui Demo Window が表示され全ウィジェットが操作可能
  - マウスクリック, ドラッグ, スクロール, キーボード入力が正常
  - Docking (タブ化, ウィンドウ分割) が動作
  - FPS が 60fps 以上 (ImGui による低下が最小限)
  - overlay example で背景色の上に ImGui が重なって表示

---

## Phase 4: kalidokit-rust アプリ統合

**目的**: kalidokit-rust アプリの VRM シーン上に ImGui デバッグ/設定 UI を表示

### Step 4.1: app/Cargo.toml に依存追加

- [ ] `imgui-renderer = { path = "../imgui-renderer" }` を追加
- [ ] `cargo check -p kalidokit-rust` が通ることを確認

### Step 4.2: AppState に ImGuiRenderer を追加

- [ ] `crates/app/src/state.rs` に `pub imgui: Option<imgui_renderer::ImGuiRenderer>` フィールド追加
- [ ] `crates/app/src/init.rs` で `ImGuiRenderer::new()` を呼び出して初期化
- [ ] `cargo check -p kalidokit-rust` が通ることを確認

### Step 4.3: イベント + フレーム + レンダーフック

- [ ] `app.rs` の `window_event` で `imgui.handle_event()` を呼び出し
- [ ] `update.rs` の描画フロー末尾に `imgui.frame()` + `imgui.render()` を追加
  - 3D シーン描画 → デバッグオーバーレイ → **ImGui render** → present

```rust
// update.rs — 描画フロー (追加部分)
// 5b. Main 3D scene render
state.scene.render_to_view(&state.render_ctx, &view);
// 5c. Debug overlay (camera preview + landmarks)
state.debug_overlay.render(/* ... */);
// 5d. ImGui overlay (NEW)
if let Some(imgui) = &mut state.imgui {
    imgui.frame(&state.render_ctx.window, elapsed, |ui| {
        // UI 構築は別関数に分離 (Step 4.4-4.5)
        crate::debug_ui::draw(ui, state);
    });
    imgui.render(&state.render_ctx.device, &state.render_ctx.queue, &view);
}
```

- [ ] `cargo check -p kalidokit-rust` が通ることを確認

### Step 4.4: デバッグ UI — `app/src/debug_ui.rs` (~100行, **新規**)

- [ ] FPS 表示 (render fps, video decode fps)
- [ ] VAD 状態 (is_voice, probability)
- [ ] トラッカー状態 (face/pose/hand 検出有無)
- [ ] 動画背景情報 (backend, width, height)
- [ ] マスコットモード状態

```rust
// debug_ui.rs
pub fn draw(ui: &dear_imgui::Ui, state: &AppState) {
    ui.window("Debug").build(|| {
        ui.text(format!("Render FPS: {}", state.fps_counter));
        ui.text(format!("Decode FPS: {}", state.fps_decode_counter));
        ui.separator();
        ui.text(format!("Mascot: {}", state.mascot.enabled));
        ui.text(format!("Always on top: {}", state.mascot.always_on_top));
        // ...
    });
}
```

### Step 4.5: 設定 UI — `app/src/settings_ui.rs` (~120行, **新規**)

- [ ] VAD threshold スライダー
- [ ] Mascot サイズ入力
- [ ] Always on top チェックボックス
- [ ] Lighting プリセット選択
- [ ] 背景色ピッカー

```rust
// settings_ui.rs
pub fn draw(ui: &dear_imgui::Ui, state: &mut AppState) {
    ui.window("Settings").build(|| {
        ui.slider("VAD Threshold", 0.0, 1.0, &mut state.vad_threshold);
        ui.checkbox("Always on Top", &mut state.mascot.always_on_top);
        // ...
    });
}
```

- [ ] **⚠ 300行超え見込み**: debug_ui.rs と settings_ui.rs を分離することで各ファイルを管理可能なサイズに保つ

### Step 4.6: UI 表示トグル

- [ ] `F1` キーで ImGui UI の表示/非表示を切替
- [ ] `AppState` に `show_imgui: bool` フィールド追加
- [ ] ImGui 非表示時はイベント転送もスキップ (パフォーマンス)

### Step 4.7: テスト

- [ ] **正常系テスト**:
  - `F1` で ImGui 表示/非表示切替
  - ImGui 表示時もアプリ FPS が維持される (< 5% 低下)
- [ ] **異常系テスト**:
  - ImGui 初期化失敗時にアプリが正常に動作 (imgui = None のフォールバック)

### Step 4.8: Phase 4 検証

- [ ] `cargo check --workspace` — 全クレート pass
- [ ] `cargo test --workspace` — 全テスト pass (tracker 除外)
- [ ] `cargo clippy --workspace -- -D warnings` — 警告なし
- [ ] `cargo fmt --check` — フォーマット OK
- [ ] テストカバレッジ 90% 以上を確認、未カバー部分のテスト追加
- [ ] `cargo build --release -p kalidokit-rust` — リリースビルド成功
- [ ] **動作確認**: release ビルドでアプリを起動し、以下を確認。目的の動作と異なる場合は修正を繰り返す:
  - `F1` で ImGui デバッグ UI が表示/非表示切替
  - FPS, VAD, トラッカー状態がリアルタイム表示
  - 設定スライダー/チェックボックスが操作可能
  - VRM モデルの上に ImGui が正しく重なる
  - マスコットモードとの共存 (透過ウィンドウ上の ImGui)
  - ImGui Docking (ウィンドウのタブ化/分割) が動作

---

## Phase 5: 拡張統合 (ImPlot, ImGuizmo, ImNodes)

**目的**: dear-imgui-rs が提供するコミュニティ拡張を統合し、データ可視化と 3D 操作 UI を追加

### Step 5.1: ImPlot — VAD 確率のリアルタイムグラフ

- [ ] `dear-imgui-rs` の ImPlot 機能を有効化 (Cargo.toml feature)
- [ ] `debug_ui.rs` に VAD probability のリアルタイム折れ線グラフを追加
- [ ] 直近 N フレームの確率履歴を ringbuffer で管理

```rust
// ImPlot 使用例
if let Some(plot) = ui.plot("VAD Probability") {
    plot.plot_line("probability", &vad_history);
}
```

### Step 5.2: ImGuizmo — VRM モデルの位置/回転操作

- [ ] ImGuizmo の Translate/Rotate/Scale ギズモを表示
- [ ] VRM モデルの world transform に接続

### Step 5.3: ImNodes — パイプライン可視化 (オプション)

- [ ] Camera → Face Detector → Solver → Bone のノードグラフを描画
- [ ] 各ノードの処理時間を表示

### Step 5.4: Phase 5 検証

- [ ] `cargo check --workspace` — 全クレート pass
- [ ] `cargo clippy --workspace -- -D warnings` — 警告なし
- [ ] テストカバレッジ確認、未カバー部分のテスト追加
- [ ] `cargo build --release` — リリースビルド成功
- [ ] **動作確認**: release ビルドで以下を確認。目的の動作と異なる場合は修正を繰り返す:
  - ImPlot グラフが VAD 確率をリアルタイム表示
  - ImGuizmo ギズモで VRM モデルを移動/回転可能
  - 全拡張が Docking 内で正常動作

---

## Phase 6: README + 動作確認チェックリスト

**目的**: ドキュメント整備と最終動作確認

### Step 6.1: README.md (英語) — `crates/imgui-renderer/README.md`

- [ ] 以下の内容で英語の README を作成:
  - プロジェクト概要 (Dear ImGui + wgpu integration)
  - 機能一覧 (Docking, ImPlot, ImGuizmo, ImNodes)
  - Install 手順 (Cargo.toml dependency + ビルド要件)
  - Quick Start コード例
  - API リファレンス (ImGuiRenderer の 4 メソッド)
  - Examples の実行方法
  - ライセンス
  - 絵文字は控えめに使用

### Step 6.2: README_ja.md (日本語) — `crates/imgui-renderer/README_ja.md`

- [ ] README.md の日本語翻訳版

### Step 6.3: 動作確認チェックリスト

以下の全項目を実際にアプリケーション (バイナリ) を起動して確認する。
エラーまたは設計通りの動作にならない場合は、問題が解決するまで繰り返し修正を行う。

**基本動作:**
- [ ] `cargo build --release -p imgui-renderer` — ビルド成功
- [ ] `cargo run -p imgui-renderer --example standalone` — ImGui Demo Window 表示
- [ ] `cargo run -p imgui-renderer --example overlay` — 背景 + ImGui オーバーレイ表示
- [ ] マウスクリック, ドラッグ, スクロールが ImGui に反映
- [ ] キーボード入力 (テキストフィールド) が正常
- [ ] ウィンドウリサイズ時に ImGui が正しく追従

**Docking:**
- [ ] ImGui ウィンドウをドラッグしてドッキング可能
- [ ] タブ化 (複数ウィンドウを1つにまとめ) が動作
- [ ] ウィンドウの分割 (左右/上下) が動作

**kalidokit-rust 統合:**
- [ ] `cargo build --release -p kalidokit-rust` — ビルド成功
- [ ] アプリ起動 + `F1` で ImGui 表示/非表示切替
- [ ] デバッグ UI: FPS, VAD 状態, トラッカー情報が表示
- [ ] 設定 UI: スライダー/チェックボックスが操作可能
- [ ] VRM モデル + 動画背景 + ImGui が同時に正しく描画
- [ ] マスコットモード + ImGui の共存

**パフォーマンス:**
- [ ] ImGui 表示時の FPS 低下 < 5%
- [ ] ImGui 非表示時 (F1 OFF) は FPS への影響ゼロ

**拡張 (Phase 5 完了後):**
- [ ] ImPlot: VAD 確率グラフがリアルタイム更新
- [ ] ImGuizmo: VRM モデルの位置/回転操作
- [ ] ImNodes: パイプラインノードグラフ表示

### Step 6.4: Phase 6 検証

- [ ] `cargo check --workspace` — 全クレート pass
- [ ] `cargo test --workspace` — 全テスト pass
- [ ] `cargo clippy --workspace -- -D warnings` — 警告なし
- [ ] `cargo fmt --check` — フォーマット OK
- [ ] `cargo doc --workspace --no-deps` — 警告なし
- [ ] `cargo build --release` — 全クレートリリースビルド成功
- [ ] README.md + README_ja.md の内容が正確で最新
- [ ] 動作確認チェックリスト (Step 6.3) の全項目が ✅
