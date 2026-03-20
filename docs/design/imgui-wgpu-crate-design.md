# imgui-wgpu ラッパークレート設計

## 1. 目的

Dear ImGui を wgpu アプリケーションに簡単に組み込めるラッパークレートを作成する。
アプリ側は wgpu の `Device`, `Queue`, `SurfaceConfiguration` を渡すだけで
ImGui の描画が統合される。

## 2. 既存エコシステムの選択肢

| クレート | ImGui バージョン | wgpu 対応 | 特徴 |
|---|---|---|---|
| `imgui` (0.12) + `imgui-wgpu` (0.28) | 1.89 | wgpu 25 | 老舗、枯れている |
| `dear-imgui-rs` (0.10) + `dear-imgui-wgpu` | 1.92.6 (docking) | wgpu 27-29 | 最新、docking 対応、マルチビューポート |
| **自前ラップ** (本計画) | 最新 (cimgui) | wgpu 24 (本プロジェクト) | 完全制御 |

### 推奨アプローチ: `dear-imgui-rs` をベースに薄いラッパーを作る

**理由:**
- `dear-imgui-rs` は ImGui 1.92.6 (docking branch) + cimgui ベースで最も新しい
- wgpu backend (`dear-imgui-wgpu`) が既にある
- 自前で cimgui の FFI を書くのは膨大な作業 (ImGui は 1000+ の API)
- 本プロジェクトの wgpu バージョン (24.0) との互換は `dear-imgui-wgpu` の feature flag で調整可能

**ただし wgpu 24 への対応が問題:**
- `dear-imgui-wgpu` は wgpu 27-29 対応
- wgpu 24 → 27 の API 差分を吸収する必要がある
- 選択肢: (A) wgpu を 27+ に上げる, (B) dear-imgui-wgpu を fork して 24 対応, (C) 自前で renderer を書く

## 3. アーキテクチャ

```
┌───────────────────────────────────────────────────┐
│  imgui-renderer (本クレート)                       │
│                                                    │
│  pub struct ImGuiRenderer {                        │
│      ctx: dear_imgui::Context,                     │
│      platform: WinitPlatform,    // winit 統合     │
│      renderer: WgpuRenderer,     // wgpu 描画      │
│  }                                                 │
│                                                    │
│  impl ImGuiRenderer {                              │
│      fn new(device, queue, format, window) -> Self │
│      fn handle_event(event)                        │
│      fn frame<F: FnOnce(&Ui)>(&mut self, f: F)    │
│      fn render(device, queue, view)                │
│  }                                                 │
└────────────┬──────────────┬───────────────────────┘
             │              │
    ┌────────▼──────┐  ┌───▼──────────────────┐
    │ dear-imgui-rs │  │ wgpu renderer        │
    │ (Context, Ui) │  │ (自前 or dear-imgui- │
    │               │  │  wgpu を fork)        │
    └───────────────┘  └──────────────────────┘
```

## 4. API 設計 — アプリ側の使い方

```rust
use imgui_renderer::ImGuiRenderer;

// 初期化 (wgpu の device/queue/format/window を渡す)
let mut imgui = ImGuiRenderer::new(
    &device,
    &queue,
    surface_config.format,
    &window,
)?;

// イベントループ内
fn window_event(&mut self, event: WindowEvent) {
    // ImGui にイベントを渡す (マウス, キーボード)
    imgui.handle_event(&window, &event);
}

// 描画
fn redraw(&mut self) {
    let output = surface.get_current_texture().unwrap();
    let view = output.texture.create_view(&Default::default());

    // ImGui フレーム: クロージャ内で UI を構築
    imgui.frame(&window, dt, |ui| {
        ui.window("Debug Panel").build(|| {
            ui.text("FPS: 60");
            ui.slider("Threshold", 0.0, 1.0, &mut threshold);
            if ui.button("Reset") { /* ... */ }
        });
    });

    // 既存の 3D シーンを描画
    scene.render_to_view(&render_ctx, &view);

    // ImGui を上に重ねて描画
    imgui.render(&device, &queue, &view);

    output.present();
}
```

## 5. クレート構成

```
crates/imgui-renderer/
├── Cargo.toml
├── src/
│   ├── lib.rs              # ImGuiRenderer — メイン API
│   ├── wgpu_backend.rs     # wgpu レンダラー (頂点バッファ, テクスチャ, パイプライン)
│   ├── winit_platform.rs   # winit イベント → ImGui 入力変換
│   └── shaders/
│       └── imgui.wgsl      # ImGui 描画用 WGSL シェーダ
└── examples/
    ├── standalone.rs        # ImGui 単体デモ (winit + wgpu)
    └── overlay.rs           # 既存 3D シーンの上に ImGui オーバーレイ
```

## 6. wgpu レンダラーの実装

ImGui の描画は以下のデータで構成される:

```
DrawData
  └── DrawList[]
        ├── VtxBuffer: Vec<ImDrawVert>   // pos(f32x2), uv(f32x2), col(u32)
        ├── IdxBuffer: Vec<ImDrawIdx>    // u16 or u32
        └── CmdBuffer: Vec<ImDrawCmd>    // clip_rect, texture_id, elem_count
```

wgpu レンダラーの責務:
1. **頂点/インデックスバッファ**: フレーム毎に `DrawVert` / `DrawIdx` を GPU にアップロード
2. **テクスチャ**: ImGui のフォントアトラス + ユーザーテクスチャを wgpu Texture に変換
3. **パイプライン**: WGSL シェーダ + ブレンドステート (Alpha Blending) + scissor rect
4. **描画**: `DrawCmd` ごとに `draw_indexed()` を発行、`clip_rect` で scissor 設定

### WGSL シェーダ

```wgsl
struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,  // u32 → vec4 に展開
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> transform: mat4x4<f32>;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = transform * vec4<f32>(in.pos, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(tex, samp, in.uv);
    return in.color * tex_color;
}
```

## 7. 2 つのアプローチの比較

### A: dear-imgui-rs + dear-imgui-wgpu を使う (推奨)

```toml
[dependencies]
dear-imgui-rs = "0.10"
dear-imgui-wgpu = { version = "0.10", features = ["wgpu-24"] }  # 要確認
dear-imgui-winit = "0.10"
```

**メリット:** ImGui API の安全なラッパーが全て揃っている
**デメリット:** wgpu 24 との互換が不明、依存が大きい

### B: imgui-rs (0.12) + 自前 wgpu レンダラー

```toml
[dependencies]
imgui = "0.12"
imgui-winit-support = "0.12"
wgpu = { workspace = true }
```

**メリット:** imgui-rs は枯れている、wgpu レンダラーだけ自前で書けば良い
**デメリット:** imgui-rs の ImGui バージョンが古い (1.89)

### C: 完全自前 (cimgui FFI + wgpu レンダラー)

```toml
[build-dependencies]
cc = "1.0"
```

**メリット:** 完全制御、最新 ImGui、最小依存
**デメリット:** 作業量が膨大 (cimgui の 1000+ API を FFI)

### 推奨: **B (imgui-rs 0.12 + 自前 wgpu レンダラー)**

理由:
- `imgui-rs` 0.12 は十分安定、API カバレッジ良好
- `imgui-wgpu` 0.28 は wgpu 25 依存で 24 とは直接互換しない → 自前レンダラーの方が確実
- wgpu レンダラーは ~300行 で実装可能 (頂点バッファ + シェーダ + テクスチャ)
- 本プロジェクトの wgpu 24.0 と直接連携可能

## 8. 実装フェーズ

### Phase 1: クレート scaffold + imgui-rs 統合
1. `crates/imgui-renderer/` 作成、ワークスペースに追加
2. `imgui = "0.12"`, `imgui-winit-support = "0.12"`, `wgpu = workspace` 依存
3. `ImGuiRenderer` struct の骨格
4. `cargo check -p imgui-renderer`

### Phase 2: wgpu レンダラー実装
1. WGSL シェーダ (§6)
2. フォントアトラステクスチャ生成
3. 頂点/インデックスバッファのフレーム毎アップロード
4. `render()` — DrawCmd → draw_indexed + scissor rect

### Phase 3: winit 統合
1. `handle_event()` — マウス, キーボード, スクロール → ImGui IO
2. `frame()` — dt 更新 + UI クロージャ呼び出し
3. カーソル形状の反映

### Phase 4: kalidokit-rust アプリ統合
1. `crates/app/` に imgui-renderer 依存追加
2. デバッグパネル: FPS, VAD 状態, トラッカー情報
3. 設定 UI: threshold, mascot size, always_on_top

### Phase 5: テスト + Example
1. `examples/standalone.rs` — ImGui 単体デモ
2. `examples/overlay.rs` — 3D シーン + ImGui オーバーレイ
3. 動作確認
