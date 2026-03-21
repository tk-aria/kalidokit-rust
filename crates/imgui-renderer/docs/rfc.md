# RFC: imgui-renderer クレート

- **作成日**: 2026-03-20
- **ステータス**: Draft
- **対象**: kalidokit-rust プロジェクト

## 概要

Dear ImGui を wgpu アプリケーションに簡単に組み込めるラッパークレートを作成する。
`dear-imgui-rs` (ImGui v1.92.6, docking branch) をベースとし、wgpu 29 レンダラーを使用する。

## 動機

1. **デバッグ UI**: VRM モデルの状態、FPS、トラッカー情報をリアルタイム表示したい
2. **設定 UI**: threshold, mascot size, lighting 等をランタイムで調整したい
3. **拡張性**: ImPlot (グラフ), ImGuizmo (3D ギズモ), ImNodes (ノードエディタ) を将来使いたい
4. **C++ エコシステム追従**: Dear ImGui のコミュニティ拡張をすぐに導入できる状態を維持したい

## 技術選定

### imgui-rs (0.12) を採用しない理由

| 問題 | 詳細 |
|------|------|
| ImGui バージョン遅れ | v1.89.2 (2023) — 本家は v1.92.6 (2025/02) |
| Docking 未完成 | optional、不安定 |
| 拡張なし | ImPlot, ImNodes, ImGuizmo 一切未対応 |
| wgpu 対応 | imgui-wgpu 0.28 → wgpu 25 (本プロジェクト 24) |
| 更新頻度 | 最終リリース 2024/05、以降活動低下 |

### dear-imgui-rs (0.10) を採用する理由

| メリット | 詳細 |
|----------|------|
| ImGui 最新 | v1.92.6 (docking branch) に同期 |
| 拡張統合済み | ImPlot, ImPlot3D, ImNodes, ImGuizmo, Test Engine, File Browser, Reflection UI |
| wgpu 29 対応 | dear-imgui-wgpu がネイティブ対応、feature flag で 27/28/29 切替 |
| FFI 自動生成 | cimgui + bindgen で C++ の変更を自動追従 |
| リリースモデル | ワークスペース協調リリース、活発な開発 |

### wgpu 24 → 29 アップグレードが必要

dear-imgui-wgpu は wgpu 29 をデフォルトとする。本プロジェクトの wgpu を 24 → 29 に上げる必要がある。

## 設計

### クレート構成

```
crates/imgui-renderer/
├── Cargo.toml
├── docs/
│   ├── rfc.md          # 本ドキュメント
│   └── sow.md          # Statement of Work
├── src/
│   └── lib.rs          # ImGuiRenderer (薄いラッパー)
└── examples/
    ├── standalone.rs    # ImGui 単体デモ (winit + wgpu)
    └── overlay.rs       # 3D シーン + ImGui オーバーレイ
```

### 依存関係

```toml
[dependencies]
dear-imgui-rs = "0.10"
dear-imgui-wgpu = "0.10"
dear-imgui-winit = "0.10"
wgpu = { workspace = true }     # 29.0
winit = { workspace = true }    # 0.30.9
log = { workspace = true }
anyhow = { workspace = true }
```

### パブリック API

```rust
/// wgpu アプリケーションに ImGui を統合するラッパー。
///
/// # Usage
/// ```rust,no_run
/// let mut imgui = ImGuiRenderer::new(&device, &queue, format, &window)?;
///
/// // イベントループ内
/// imgui.handle_event(&window, &event);
///
/// // 描画
/// imgui.frame(&window, dt, |ui| {
///     ui.window("Debug").build(|| {
///         ui.text("Hello, ImGui!");
///     });
/// });
/// scene.render_to_view(&ctx, &view);  // 3D シーン
/// imgui.render(&device, &queue, &view);  // ImGui を上に重ねる
/// ```
pub struct ImGuiRenderer {
    ctx: dear_imgui::Context,
    platform: dear_imgui_winit::WinitPlatform,
    renderer: dear_imgui_wgpu::Renderer,
}

impl ImGuiRenderer {
    /// wgpu Device/Queue/TextureFormat + winit Window から初期化。
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        window: &winit::window::Window,
    ) -> anyhow::Result<Self>;

    /// winit イベントを ImGui に転送 (マウス, キーボード, スクロール)。
    pub fn handle_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    );

    /// ImGui フレームを開始し、クロージャ内で UI を構築。
    pub fn frame<F: FnOnce(&dear_imgui::Ui)>(
        &mut self,
        window: &winit::window::Window,
        dt: std::time::Duration,
        f: F,
    );

    /// ImGui の描画コマンドを wgpu RenderPass として発行。
    /// 既存の 3D シーンの上に重ねて描画する (LoadOp::Load)。
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
    );

    /// ImGui Context への直接アクセス (高度な設定用)。
    pub fn context_mut(&mut self) -> &mut dear_imgui::Context;
}
```

### データフロー

```
winit::WindowEvent
  │
  ▼
ImGuiRenderer::handle_event()
  │ マウス/キーボード → ImGui IO
  ▼
ImGuiRenderer::frame(|ui| { ... })
  │ UI 構築 → DrawList 生成
  ▼
ImGuiRenderer::render()
  │ DrawList → wgpu RenderPass
  │ 頂点バッファ + テクスチャ + scissor → draw_indexed
  ▼
wgpu Surface Present
```

## 前提条件

- wgpu 24 → 29 へのアップグレードが完了していること (Phase A)
- winit 0.30.9 (既存バージョン、変更なし)

## リスク

| リスク | 影響 | 対策 |
|--------|------|------|
| wgpu 24→29 の API 差分が大きい | renderer クレートの修正量大 | 段階的修正、クレートごとにチェック |
| dear-imgui-wgpu が winit 0.30 と非互換 | ビルドエラー | dear-imgui-winit の winit バージョン確認、必要なら fork |
| dear-imgui-rs の API が不安定 | 将来の破壊的変更 | バージョン固定、changelog 監視 |
| ImGui のメモリモデルが Rust と合わない | unsafe 多用 | dear-imgui-rs が安全な API を提供、raw API は使わない |

## 代替案

1. **imgui-rs 0.12 + 自前 wgpu レンダラー**: wgpu 24 のまま使えるが、ImGui 古い + 拡張なし → 却下
2. **egui**: Rust ネイティブ GUI だが ImGui のエコシステム (C++ 拡張) が使えない → 却下
3. **自前 cimgui FFI**: 完全制御だが 1000+ API の FFI 作業が膨大 → 却下
