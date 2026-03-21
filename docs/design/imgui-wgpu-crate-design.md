# imgui-wgpu ラッパークレート設計 (dear-imgui-rs ベース)

## 1. 方針変更

`imgui-rs` (ImGui 1.89, 2 年遅れ, 拡張なし) から **`dear-imgui-rs`** (ImGui 1.92.6 最新, docking, ImPlot/ImNodes/ImGuizmo 統合) に変更。

wgpu を **24 → 29** にアップグレードし、`dear-imgui-wgpu` (wgpu 29 対応) をそのまま使用。

## 2. ライブラリ構成

| クレート | バージョン | 役割 |
|---|---|---|
| `dear-imgui-rs` | 0.10 | ImGui 1.92.6 安全な Rust API (docking, 拡張込み) |
| `dear-imgui-sys` | 0.10 | cimgui FFI (bindgen 生成) |
| `dear-imgui-wgpu` | 0.10 | wgpu 29 レンダラーバックエンド |
| `dear-imgui-winit` | 0.10 | winit プラットフォームバックエンド |
| `wgpu` | **29.0** | GPU API (24 から上げる) |
| `winit` | 0.30.9 | ウィンドウ管理 (既存) |

### 利用可能な拡張 (dear-imgui-rs 経由)

| 拡張 | 用途 |
|---|---|
| **ImPlot / ImPlot3D** | グラフ・プロット描画 |
| **ImNodes** | ノードエディタ |
| **ImGuizmo** | 3D ギズモ (移動/回転/拡大) |
| **Test Engine** | UI 自動テスト |
| **File Browser** | ファイル選択ダイアログ |
| **Reflection UI** | Rust struct から自動 UI 生成 |

## 3. wgpu 24 → 29 アップグレード

### 影響範囲

| クレート | wgpu 使用 | 変更量 |
|---|---|---|
| `renderer` | 大量 (Surface, Device, Queue, Pipeline, Texture, RenderPass) | **大** |
| `video-decoder` | Texture, Queue (write_texture, compute) | 中 |
| `app` | Surface acquire, present | 小 |

### wgpu 24 → 29 の主な破壊的変更

```
24 → 25: BufferAddress → u64, TextureDescriptor 変更
25 → 26: RenderPassDescriptor lifetime 変更
26 → 27: Instance::new() シグネチャ変更
27 → 28: SurfaceConfiguration 変更
28 → 29: BindGroupLayout 自動推論改善, Error 型変更
```

### アップグレード戦略

1. `Cargo.toml` の workspace dependency を `wgpu = "29.0"` に変更
2. `cargo check --workspace` でエラーを列挙
3. クレートごとに API 差分を修正
4. 全テスト pass を確認

## 4. imgui-renderer クレート設計

```
crates/imgui-renderer/
├── Cargo.toml
├── src/
│   └── lib.rs            # ImGuiRenderer (薄いラッパー)
└── examples/
    ├── standalone.rs      # ImGui 単体デモ
    └── overlay.rs         # 3D シーン + ImGui オーバーレイ
```

### Cargo.toml

```toml
[package]
name = "imgui-renderer"
version = "0.1.0"
edition = "2021"

[dependencies]
dear-imgui-rs = "0.10"
dear-imgui-wgpu = "0.10"       # wgpu 29 レンダラー
dear-imgui-winit = "0.10"      # winit プラットフォーム
wgpu = { workspace = true }     # 29.0
winit = { workspace = true }
log = { workspace = true }
```

### API

```rust
use imgui_renderer::ImGuiRenderer;

// 初期化
let mut imgui = ImGuiRenderer::new(&device, &queue, surface_format, &window)?;

// イベント処理
imgui.handle_event(&window, &event);

// 描画
imgui.frame(&window, dt, |ui| {
    ui.window("Debug").build(|| {
        ui.text("Hello, ImGui!");
        ui.text(format!("FPS: {:.0}", fps));

        // ImPlot (拡張)
        if let Some(plot) = ui.plot("Audio") {
            plot.plot_line("VAD", &vad_history);
        }

        // ImGuizmo (拡張)
        // ui.gizmo(...);
    });
});

// 3D シーン描画後に ImGui を重ねる
imgui.render(&device, &queue, &view);
```

### lib.rs

```rust
pub struct ImGuiRenderer {
    ctx: dear_imgui::Context,
    platform: dear_imgui_winit::WinitPlatform,
    renderer: dear_imgui_wgpu::Renderer,
}

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
        platform.attach_window(&mut ctx, window, dear_imgui_winit::HiDpiMode::Default);

        let renderer = dear_imgui_wgpu::Renderer::new(&mut ctx, device, queue, format);

        Ok(Self { ctx, platform, renderer })
    }

    pub fn handle_event(&mut self, window: &winit::window::Window, event: &winit::event::WindowEvent) {
        self.platform.handle_event(&mut self.ctx, window, event);
    }

    pub fn frame<F: FnOnce(&dear_imgui::Ui)>(
        &mut self,
        window: &winit::window::Window,
        dt: std::time::Duration,
        f: F,
    ) {
        self.ctx.io_mut().update_delta_time(dt);
        self.platform.prepare_frame(&mut self.ctx, window).unwrap();
        let ui = self.ctx.frame();
        f(ui);
        self.platform.prepare_render(ui, window);
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
    ) {
        let draw_data = self.ctx.render();
        let mut encoder = device.create_command_encoder(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("imgui_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,  // 既存シーンを保持
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        self.renderer.render(draw_data, queue, device, &mut pass).unwrap();
        drop(pass);
        queue.submit(std::iter::once(encoder.finish()));
    }
}
```

## 5. 実装フェーズ

### Phase A: wgpu 24 → 29 アップグレード
1. `Cargo.toml` で `wgpu = "29.0"` に変更
2. `crates/renderer/` の API 差分を修正 (最大変更量)
3. `crates/video-decoder/` の API 差分を修正
4. `crates/app/` の API 差分を修正
5. `cargo check --workspace` pass
6. `cargo test --workspace` pass
7. 動作確認 (VRM + 動画背景)

### Phase B: imgui-renderer クレート作成
1. `crates/imgui-renderer/` scaffold
2. `ImGuiRenderer` 実装 (dear-imgui-rs + dear-imgui-wgpu + dear-imgui-winit)
3. `examples/standalone.rs` — ImGui 単体デモ
4. 動作確認

### Phase C: kalidokit-rust アプリ統合
1. `crates/app/` に imgui-renderer 依存追加
2. デバッグパネル UI (FPS, VAD, トラッカー)
3. 設定 UI (threshold, mascot, lighting)
4. 動作確認

### Phase D: 拡張統合 (必要に応じて)
1. ImPlot — VAD 確率のリアルタイムグラフ
2. ImGuizmo — VRM モデルの位置/回転操作
3. ImNodes — トラッカーパイプラインの可視化
