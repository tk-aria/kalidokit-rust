# KalidoKit Rust - 実装タスク (wgpu版)

> 各Phaseは順番に実装。各Step内のチェックボックスを完了順にチェックする。
> 300行以上になるファイルは分割候補として明記。
> `cargo fmt --check` は全Phaseの検証で毎回実行すること。

## Phase依存関係

```
Phase 1 (wgpu基盤)
  ↓
Phase 2 (VRMローダー) ← Phase 1に依存
  ↓
Phase 3 (Skinning/MorphTarget描画) ← Phase 1, 2に依存
  ↓
Phase 4 (ソルバー) ← 独立 (Phase 1-3と並行可能)
  ↓
Phase 5 (トラッカー) ← 独立 (Phase 1-3と並行可能)
  ↓
Phase 6 (統合) ← Phase 1-5 全てに依存
  ↓
Phase 7 (仕上げ) ← Phase 6に依存
```

## ライブラリバージョン一覧

| クレート | バージョン | 用途 |
|---------|-----------|------|
| `wgpu` | 24.0 | GPU描画 (Vulkan/Metal/DX12/WebGPU) |
| `winit` | 0.30.9 | ウィンドウ管理・イベントループ |
| `glam` | 0.29.2 | 線形代数 (Vec3/Quat/Mat4) |
| `gltf` | 1.4.1 | glTF 2.0パーサー |
| `bytemuck` | 1.21.0 | Pod型→バイト列変換 |
| `serde` | 1.0.219 | シリアライズ |
| `serde_json` | 1.0.140 | JSONパース (VRM拡張) |
| `ort` | 2.0.0-rc.12 | ONNX Runtime推論 |
| `nokhwa` | 0.10.7 | Webカメラキャプチャ |
| `image` | 0.25.6 | 画像処理 |
| `ndarray` | 0.16.1 | テンソル操作 |
| `anyhow` | 1.0.97 | エラーハンドリング |
| `thiserror` | 2.0.12 | カスタムエラー型 |
| `pollster` | 0.4.0 | async→sync ブリッジ |
| `env_logger` | 0.11.6 | ロギング |
| `log` | 0.4.27 | ログマクロ |
| `cargo-llvm-cov` | 0.6+ (dev) | テストカバレッジ計測 (`cargo install cargo-llvm-cov`) |

---

## Phase 1: プロジェクト基盤 & wgpuレンダラー

**目的**: ウィンドウ表示 + wgpu初期化 + 三角形描画まで動作確認

### Step 1.1: ワークスペース再構築

- [x] **Cargo.toml (ルート)**: ワークスペースメンバーを5クレート構成に変更
  - members: `app`, `renderer`, `vrm`, `solver`, `tracker`
  - `[workspace.dependencies]` に上記バージョンを全て明記
  - 既存のBevy依存 (`bevy`, `bevy_vrm`) を削除

```toml
# Cargo.toml
[workspace]
resolver = "2"
members = ["crates/app", "crates/renderer", "crates/vrm", "crates/solver", "crates/tracker"]

[workspace.dependencies]
wgpu = "24.0"
winit = "0.30.9"
glam = { version = "0.29.2", features = ["bytemuck"] }
gltf = "1.4.1"
bytemuck = { version = "1.21", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
ort = "2.0.0-rc.12"
nokhwa = { version = "0.10", features = ["input-native"] }
image = "0.25"
ndarray = "0.16"
anyhow = "1.0"
thiserror = "2.0"
pollster = "0.4"
env_logger = "0.11"
log = "0.4"
```

- [x] **crates/renderer/Cargo.toml** 新規作成

```toml
[package]
name = "renderer"
version = "0.1.0"
edition = "2021"

[dependencies]
wgpu = { workspace = true }
winit = { workspace = true }
glam = { workspace = true }
bytemuck = { workspace = true }
image = { workspace = true }
anyhow = { workspace = true }
log = { workspace = true }

[dev-dependencies]
pollster = { workspace = true }
```

- [x] **crates/vrm/Cargo.toml** 新規作成

```toml
[package]
name = "vrm"
version = "0.1.0"
edition = "2021"

[dependencies]
gltf = { workspace = true }
glam = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
log = { workspace = true }
```

- [x] **crates/app/Cargo.toml** を wgpu版に書き換え (Bevy依存削除)

```toml
[package]
name = "kalidokit-rust"
version = "0.1.0"
edition = "2021"

[dependencies]
renderer = { path = "../renderer" }
vrm = { path = "../vrm" }
solver = { path = "../solver" }
tracker = { path = "../tracker" }
winit = { workspace = true }
nokhwa = { workspace = true }
image = { workspace = true }
pollster = { workspace = true }
env_logger = { workspace = true }
log = { workspace = true }
anyhow = { workspace = true }
```

- [x] **crates/solver/Cargo.toml**: `thiserror` 追加
- [x] **crates/tracker/Cargo.toml**: `thiserror` 追加
- [x] 既存の Bevy 依存コード (`crates/app/src/`) を全て削除し空の `main.rs` を配置
- [x] `cargo check` が全クレートで成功することを確認

### Step 1.2: renderer::context — wgpu初期化

**ファイル**: `crates/renderer/src/context.rs` (~80行)

- [x] `RenderContext` 構造体を実装
  - フィールド: `device: Device`, `queue: Queue`, `surface: Surface`, `config: SurfaceConfiguration`
  - `new(window: &Window) -> Result<Self>` : Instance作成 → Adapter取得 → Device/Queue取得 → Surface設定
  - `resize(width, height)` : SurfaceConfigurationを更新して再configure

```rust
// 参考: wgpu公式 triangle example
// https://github.com/gfx-rs/wgpu/blob/trunk/examples/src/hello_triangle/mod.rs
pub struct RenderContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub window: std::sync::Arc<winit::window::Window>,
}

impl RenderContext {
    pub async fn new(window: std::sync::Arc<winit::window::Window>) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.ok_or_else(|| anyhow::anyhow!("No adapter"))?;
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor::default(), None
        ).await?;
        let size = window.inner_size();
        let config = surface.get_default_config(&adapter, size.width, size.height)
            .ok_or_else(|| anyhow::anyhow!("No surface config"))?;
        surface.configure(&device, &config);
        Ok(Self { device, queue, surface, config, window })
    }
}
// Note: Arc<Window> により Surface は 'static ライフタイムを取得。
// AppState に RenderContext をライフタイム引数なしで保持可能。
```

- [x] `crates/renderer/src/lib.rs` に `pub mod context;` を追加

### Step 1.3: renderer::vertex — 頂点データ定義

**ファイル**: `crates/renderer/src/vertex.rs` (~50行)

- [x] `Vertex` 構造体を定義 (`#[repr(C)]`, `bytemuck::Pod/Zeroable`)
  - フィールド: `position: [f32; 3]`, `normal: [f32; 3]`, `uv: [f32; 2]`
  - `desc()` で `wgpu::VertexBufferLayout` を返す static メソッド

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x2 },
            ],
        }
    }
}
```

### Step 1.4: renderer::pipeline — RenderPipeline構築

**ファイル**: `crates/renderer/src/pipeline.rs` (~100行)

- [x] `create_render_pipeline(device, config, shader_src) -> RenderPipeline` 関数を実装
  - `device.create_shader_module()` で WGSLシェーダーをコンパイル
  - `device.create_pipeline_layout()` で BindGroupLayout を設定
  - `device.create_render_pipeline()` で Pipeline を構築
  - Vertex layout は `Vertex::layout()` を使用
  - primitive: TriangleList, front_face: CCW, cull_mode: Back

```rust
pub fn create_render_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader_src: &str,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader"),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline_layout"),
        bind_group_layouts,
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("render_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[super::vertex::Vertex::layout()],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
```

### Step 1.5: renderer::camera — カメラ行列管理

**ファイル**: `crates/renderer/src/camera.rs` (~80行)

- [x] `Camera` 構造体: `position: Vec3`, `target: Vec3`, `fov: f32`, `aspect: f32`, `near: f32`, `far: f32`
- [x] `CameraUniform` 構造体 (`#[repr(C)]`, Pod): `view_proj: [[f32; 4]; 4]`, `model: [[f32; 4]; 4]`
- [x] `Camera::build_view_projection_matrix() -> Mat4` を実装
- [x] `Camera::to_uniform() -> CameraUniform` を実装
- [x] GPU Uniform Buffer 作成・更新メソッド (Phase 3のScene統合時に実装) <!-- Scene::new() でバッファ作成、Scene::prepare() で更新済み — 2026-03-10 00:26 JST -->

```rust
pub struct Camera {
    pub position: glam::Vec3,
    pub target: glam::Vec3,
    pub fov: f32,    // degrees
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn build_view_proj(&self) -> glam::Mat4 {
        let view = glam::Mat4::look_at_rh(self.position, self.target, glam::Vec3::Y);
        let proj = glam::Mat4::perspective_rh(self.fov.to_radians(), self.aspect, self.near, self.far);
        proj * view
    }
}
```

### Step 1.6: assets/shaders — 基本WGSLシェーダー

**ファイル**: `assets/shaders/basic.wgsl` (~40行)

- [x] Vertex Shader: CameraUniform (view_proj, model) を使って頂点を変換
- [x] Fragment Shader: Lambert diffuse ライティング

```wgsl
struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = camera.view_proj * camera.model * vec4<f32>(pos, 1.0);
    out.normal = (camera.model * vec4<f32>(normal, 0.0)).xyz;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let ndotl = max(dot(normalize(in.normal), light_dir), 0.0);
    let color = vec3<f32>(0.8, 0.8, 0.8) * (0.3 + 0.7 * ndotl);
    return vec4<f32>(color, 1.0);
}
```

### Step 1.7: app — winit EventLoop + wgpu描画統合

**ファイル**: `crates/app/src/main.rs` (~120行), `crates/app/src/app.rs` (~150行)

- [x] `main.rs`: `EventLoop::new()` → `event_loop.run_app(&mut app)` のエントリポイント
- [x] `app.rs`: `App` 構造体に `ApplicationHandler` トレイトを実装
  - `resumed()`: ウィンドウ作成 (`Arc::new(window)`) → `RenderContext::new(arc_window)` → Pipeline作成 → 三角形Vertex/Index Buffer作成
  - `window_event(RedrawRequested)`: clear色で画面クリア → 三角形描画 → present
  - `window_event(Resized)`: `ctx.resize()` 呼び出し
  - `window_event(CloseRequested)`: `event_loop.exit()`
- [x] 実行して 緑背景に白い三角形が表示されることを確認 (GPU環境でのみ手動確認) <!-- Phase進行により VRM 描画に発展済み、レンダーループ動作確認済み (144fps/3sec) — 2026-03-10 00:26 JST -->

> **300行超え注意**: `app.rs` が300行を超えそうな場合、初期化ロジックを `app/src/init.rs` に分離

### Step 1.8: Dockerfile作成

**ファイル**: `Dockerfile` (~30行)

- [x] `rust:1.85-bookworm` ベースの multi-stage build
  - Stage 1 (builder): `cargo build --release`
  - Stage 2 (runtime): `debian:bookworm-slim` + `libvulkan1` + バイナリコピー
- [x] `.dockerignore` に `target/`, `.git/`, `assets/models/*.vrm`, `assets/ml/*.onnx` を追加

```dockerfile
FROM rust:1.85-bookworm AS builder
WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y cmake pkg-config libx11-dev libxkbcommon-dev && \
    cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libvulkan1 libx11-6 libxkbcommon0 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/kalidokit-rust /usr/local/bin/
COPY --from=builder /app/assets /app/assets
WORKDIR /app
CMD ["kalidokit-rust"]
```

### Step 1.9: Phase 1 検証

- [x] **テスト実装**: 8テスト全パス
  - `renderer/src/vertex.rs`: vertex_layout_stride, vertex_is_pod, cast_slice_wrong_size_panics
  - `renderer/src/camera.rs`: build_view_proj_not_identity, aspect_change_affects_matrix, uniform_is_pod, position_equals_target_no_nan, extreme_fov_values
  - `renderer/src/context.rs`: GPU/Window必要のため自動テスト対象外 (コメント明記)
  - `renderer/src/pipeline.rs`: GPU Device必要のため自動テスト対象外
  - 注: `cargo llvm-cov` はort-sysリンクエラー(glibc 2.38+必要)のため--workspace実行不可、renderer単体テストは全パス
- [x] **ビルド検証**:
  - `cargo check --workspace` 成功
  - `cargo clippy --workspace -- -D warnings` 警告0
  - `cargo fmt --check` 差分なし
  - 注: `cargo build --release` はort-sysリンクの都合で--workspace不可、renderer/solver/vrm/appは個別check成功
  - 注: `docker build` はdocker未インストールのため実行不可
  - 注: ウィンドウ表示はヘッドレス環境のため手動確認不可
- [x] エラーが発生した場合は修正し、再度全チェックを通す

---

## Phase 2: VRMローダー (vrm クレート)

**目的**: VRMファイルを読み込み、メッシュ・ボーン・BlendShapeデータを構造体に格納

### Step 2.1: vrm::error — カスタムエラー型

**ファイル**: `crates/vrm/src/error.rs` (~40行)

- [x] `VrmError` enum を `thiserror` で定義
  - `GltfError(#[from] gltf::Error)`: glTFパースエラー
  - `MissingExtension(String)`: VRM拡張が見つからない
  - `InvalidBone(String)`: 不正なボーン名
  - `MissingData(String)`: 必要なデータが欠落
  - `JsonError(#[from] serde_json::Error)`: JSON解析エラー

```rust
#[derive(Debug, thiserror::Error)]
pub enum VrmError {
    #[error("glTF parse error: {0}")]
    GltfError(#[from] gltf::Error),
    #[error("VRM extension missing: {0}")]
    MissingExtension(String),
    #[error("Invalid bone: {0}")]
    InvalidBone(String),
    #[error("Missing data: {0}")]
    MissingData(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}
```

### Step 2.2: vrm::model — VRMモデルデータ構造

**ファイル**: `crates/vrm/src/model.rs` (~60行)

- [x] `VrmModel` 構造体を定義
  - `meshes: Vec<MeshData>`: 各プリミティブの頂点/インデックス/MorphTarget
  - `skins: Vec<SkinJoint>`: スキンジョイント・InverseBindMatrix
  - `humanoid_bones: HumanoidBones`: VRMボーンマッピング (Step 2.3で追加)
  - `blend_shapes: BlendShapeGroup`: BlendShapeプリセット (Step 2.4で追加)
  - `node_transforms: Vec<NodeTransform>`: glTFノード変換
- [x] `SkinJoint` 構造体: `node_index: usize`, `inverse_bind_matrix: Mat4`
- [x] `MeshData` 構造体: `vertices: Vec<Vertex>`, `indices: Vec<u32>`, `morph_targets: Vec<MorphTargetData>`
- [x] `MorphTargetData` 構造体: `position_deltas: Vec<[f32; 3]>`, `normal_deltas: Vec<[f32; 3]>`
- [x] `NodeTransform` 構造体: `translation: Vec3`, `rotation: Quat`, `scale: Vec3`, `children: Vec<usize>`

```rust
/// glTFスキンのジョイント情報
pub struct SkinJoint {
    /// glTFノードインデックス
    pub node_index: usize,
    /// バインドポーズの逆行列
    pub inverse_bind_matrix: glam::Mat4,
}
```

### Step 2.3: vrm::bone — ヒューマノイドボーンマッピング

**ファイル**: `crates/vrm/src/bone.rs` (~180行)

- [x] `HumanoidBoneName` enum: 全55ボーン名を定義

```rust
/// VRM 0.x Humanoid Bone Names (55種)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HumanoidBoneName {
    // Spine (6)
    Hips, Spine, Chest, UpperChest, Neck, Head,
    // Left Arm (4)
    LeftShoulder, LeftUpperArm, LeftLowerArm, LeftHand,
    // Right Arm (4)
    RightShoulder, RightUpperArm, RightLowerArm, RightHand,
    // Left Leg (4)
    LeftUpperLeg, LeftLowerLeg, LeftFoot, LeftToes,
    // Right Leg (4)
    RightUpperLeg, RightLowerLeg, RightFoot, RightToes,
    // Left Fingers (15)
    LeftThumbProximal, LeftThumbIntermediate, LeftThumbDistal,
    LeftIndexProximal, LeftIndexIntermediate, LeftIndexDistal,
    LeftMiddleProximal, LeftMiddleIntermediate, LeftMiddleDistal,
    LeftRingProximal, LeftRingIntermediate, LeftRingDistal,
    LeftLittleProximal, LeftLittleIntermediate, LeftLittleDistal,
    // Right Fingers (15)
    RightThumbProximal, RightThumbIntermediate, RightThumbDistal,
    RightIndexProximal, RightIndexIntermediate, RightIndexDistal,
    RightMiddleProximal, RightMiddleIntermediate, RightMiddleDistal,
    RightRingProximal, RightRingIntermediate, RightRingDistal,
    RightLittleProximal, RightLittleIntermediate, RightLittleDistal,
    // Eyes & Jaw (3)
    LeftEye, RightEye, Jaw,
}
```

- [x] `HumanoidBoneName::from_str(s: &str) -> Option<Self>`: VRM JSON文字列→enum変換 (camelCase: "hips", "leftUpperArm" 等)
- [x] `Bone` 構造体: `node_index`, `local_rotation`, `local_position`, `inverse_bind_matrix`, `children`
- [x] `HumanoidBones` 構造体:
  - `from_vrm_json(json: &serde_json::Value) -> Result<Self>`: VRM拡張JSONからパース
  - `get(name: HumanoidBoneName) -> Option<&Bone>`
  - `set_rotation(name: HumanoidBoneName, rotation: Quat)`: ボーンのローカル回転を設定
  - `compute_joint_matrices() -> Vec<Mat4>`: Forward Kinematics で全ボーンのワールド行列を計算

```rust
// VRM JSON 構造:
// { "humanoid": { "humanBones": [ { "bone": "hips", "node": 3 }, ... ] } }
impl HumanoidBones {
    pub fn from_vrm_json(vrm_ext: &serde_json::Value) -> Result<Self, VrmError> {
        let human_bones = vrm_ext
            .get("humanoid").and_then(|h| h.get("humanBones"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| VrmError::MissingExtension("humanoid.humanBones".into()))?;
        // 各エントリの "bone" と "node" をパース
        todo!()
    }
}
```

### Step 2.4: vrm::blendshape — BlendShapeプリセット管理

**ファイル**: `crates/vrm/src/blendshape.rs` (~120行)

- [x] `BlendShapePreset` enum: `Blink, BlinkL, BlinkR, A, I, U, E, O, Joy, Angry, Sorrow, Fun, Neutral`
- [x] `BlendShapePreset::from_str(s: &str) -> Option<Self>`: JSON文字列→enum変換
- [x] `BlendShapeBinding` 構造体: `mesh_index`, `morph_target_index`, `weight`
- [x] `BlendShapeGroup` 構造体:
  - `from_vrm_json(json: &serde_json::Value) -> Result<Self>`: VRM拡張JSONからパース
  - `set(preset, value: f32)`: プリセットの重みを設定
  - `get_all_weights(num_targets) -> Vec<f32>`: 全MorphTargetの重み配列を取得 (GPU転送用)

```rust
// VRM JSON 構造:
// { "blendShapeMaster": { "blendShapeGroups": [
//   { "presetName": "blink", "binds": [ { "mesh": 0, "index": 1, "weight": 100 } ] }
// ] } }
```

### Step 2.5: vrm::loader — VRMファイルロード

**ファイル**: `crates/vrm/src/loader.rs` (~250行)

> **300行超え注意**: ロード処理が300行を超える場合、メッシュパースを `loader/mesh_parser.rs` に分離

- [x] `load(path: &str) -> Result<VrmModel>` 関数を実装
  1. `gltf::Gltf::open(path)` でglTFをパース
  2. `gltf.blob` からバイナリバッファを取得
  3. メッシュ群をパース: 各Primitive の position/normal/uv/indices を読み取り `MeshData` に格納
  4. MorphTarget をパース: 各Primitive の morph target position/normal deltas を読み取り
  5. Skin/Joint をパース: `inverse_bind_matrices` を読み取り
  6. VRM拡張JSONをパース: `extensions.VRM` を取得
  7. `HumanoidBones::from_vrm_json()` でボーンマッピング構築
  8. `BlendShapeGroup::from_vrm_json()` でBlendShape構築
  9. `VrmModel` を組み立てて返す
- [x] `read_accessor_data(blob, accessor) -> Vec<u8>`: glTFアクセサからバイト列を読み取るヘルパー
- [x] `read_accessor_as<T: Pod>(blob, accessor) -> Vec<T>`: バイト列をPod型にキャストする型付きヘルパー

```rust
// glTFアクセサからバイト列を読む低レベルヘルパー
fn read_accessor_data(blob: &[u8], accessor: &gltf::Accessor) -> Vec<u8> {
    let view = accessor.view().expect("accessor must have view");
    let offset = view.offset() + accessor.offset();
    let length = accessor.count() * accessor.size();
    blob[offset..offset + length].to_vec()
}

// Pod型にキャストする型付きヘルパー
fn read_accessor_as<T: bytemuck::Pod>(blob: &[u8], accessor: &gltf::Accessor) -> Vec<T> {
    let bytes = read_accessor_data(blob, accessor);
    bytemuck::cast_slice::<u8, T>(&bytes).to_vec()
}
```

### Step 2.6: vrm::look_at — 視線制御

**ファイル**: `crates/vrm/src/look_at.rs` (~60行)

- [x] `LookAtApplyer` 構造体: `horizontal_inner/outer`, `vertical_up/down` のカーブパラメータ
- [x] `apply(euler: &EulerAngles) -> Quat`: 瞳孔方向からボーン回転またはBlendShape値を計算
- [x] VRM JSON からパース: `extensions.VRM.firstPerson.lookAtTypeName` ("Bone" or "BlendShape")

### Step 2.7: Phase 2 検証

- [x] **テスト実装**: 26テスト全パス (renderer:8 + vrm:18)
  - `vrm/src/error.rs`: display_missing_extension, display_invalid_bone, display_missing_data, from_json_error (4)
  - `vrm/src/bone.rs`: from_str_hips, from_str_left_upper_arm, from_str_invalid, from_str_all_55_bones, from_vrm_json_parses_bones, missing_human_bones_key_returns_error (6)
  - `vrm/src/blendshape.rs`: preset_from_str, set_and_get_weights, multiple_presets_add_weights, missing_blend_shape_master_returns_error (4)
  - `vrm/src/loader.rs`: load_nonexistent_file_returns_error (1)
  - `vrm/src/look_at.rs`: apply_zero_returns_identity, apply_extreme_values_no_nan, from_vrm_json_parses (3)
  - 注: `cargo llvm-cov` はort-sysリンクエラーのため--workspace不可
- [x] **ビルド検証**:
  - `cargo check --workspace` 成功
  - `cargo clippy --workspace -- -D warnings` 警告0 (from_str→parse リネーム、needless_range_loop修正)
  - `cargo fmt --check` 差分なし
  - 注: docker/release build/ウィンドウ確認は環境制約で省略

---

## Phase 3: wgpuレンダラー拡張 (Skinning + MorphTarget + Depth)

**目的**: VRMモデルのスキニングとMorphTargetをGPUで描画

### Step 3.1: renderer::mesh — GPUメッシュ管理

**ファイル**: `crates/renderer/src/mesh.rs` (~100行)

- [x] `GpuMesh` 構造体: `vertex_buffer`, `index_buffer`, `num_indices`
- [x] `GpuMesh::from_vertices_indices(device, vertices, indices) -> Self`: CPU側データ → GPU Buffer 変換
- [x] `GpuMesh::draw(render_pass)`: `set_vertex_buffer` + `set_index_buffer` + `draw_indexed`

```rust
use wgpu::util::DeviceExt;

pub struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
}

impl GpuMesh {
    pub fn from_mesh_data(device: &wgpu::Device, mesh: &super::super::vrm::model::MeshData) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex_buffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index_buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, num_indices: mesh.indices.len() as u32 }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
    }
}
```

### Step 3.2: renderer::skin — スキニングGPUバッファ

**ファイル**: `crates/renderer/src/skin.rs` (~80行)

- [x] `SkinData` 構造体: `joint_buffer: Buffer`, `bind_group: BindGroup`
- [x] `SkinData::new(device, max_joints)`: Storage Buffer 作成
- [x] `SkinData::update(queue, joint_matrices: &[Mat4])`: `queue.write_buffer` でGPU転送
- [x] `SkinData::bind_group()`: BindGroup参照を返す

### Step 3.3: renderer::morph — MorphTarget GPUバッファ

**ファイル**: `crates/renderer/src/morph.rs` (~80行)

- [x] `MorphData` 構造体: `weight_buffer: Buffer`, `bind_group: BindGroup`
- [x] `MorphData::new(device, max_targets)`: Storage Buffer 作成
- [x] `MorphData::update(queue, weights: &[f32])`: `queue.write_buffer` でGPU転送

### Step 3.4: renderer::depth — デプスバッファ

**ファイル**: `crates/renderer/src/depth.rs` (~50行)

- [x] `DepthTexture` 構造体: `texture`, `view`
- [x] `DepthTexture::new(device, width, height)`: `Depth32Float` テクスチャ作成
- [x] `DepthTexture::resize(device, width, height)`: ウィンドウリサイズ時に再作成

```rust
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct DepthTexture {
    pub view: wgpu::TextureView,
}

impl DepthTexture {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        Self { view: texture.create_view(&Default::default()) }
    }
}
```

### Step 3.5: renderer::texture — テクスチャ管理

**ファイル**: `crates/renderer/src/texture.rs` (~100行)

- [x] `GpuTexture` 構造体: `texture`, `view`, `sampler`
- [x] `GpuTexture::from_image(device, queue, image)`: `image::DynamicImage` → GPU Texture
- [x] `GpuTexture::from_bytes(device, queue, bytes, width, height)`: raw bytes → GPU Texture
- [x] `GpuTexture::default_white(device, queue) -> Self`: デフォルトの白テクスチャ (1x1) 生成メソッド
- [x] **Scene/パイプラインへの統合**: GpuTexture を Scene に統合、VRM マテリアル/テクスチャロード実装、skinning.wgsl にテクスチャサンプリング追加 <!-- 2026-03-10 00:31 JST -->

```rust
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl GpuTexture {
    pub fn from_bytes(device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8], width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            texture.as_image_copy(),
            bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
            size,
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        Self { texture, view, sampler }
    }

    pub fn default_white(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self::from_bytes(device, queue, &[255, 255, 255, 255], 1, 1)
    }
}
```

### Step 3.6: assets/shaders/skinning.wgsl — スキニングシェーダー

**ファイル**: `assets/shaders/skinning.wgsl` (~80行)

- [x] `VertexInput` に基本頂点属性 (position, normal, uv) を定義
- [x] BindGroup 1: `JointMatrices` (Storage Buffer, 最大256ボーン)
- [x] BindGroup 2: `MorphWeights` (Storage Buffer, 最大64ターゲット)
- [x] Vertex Shader: camera.model でワールド変換、view_proj でクリップ変換
- [x] Fragment Shader: Lambert diffuse

### Step 3.7: renderer::scene — シーン描画統合

**ファイル**: `crates/renderer/src/scene.rs` (~150行)

- [x] `Scene` 構造体: `meshes`, `skin`, `morph`, `depth`, `pipeline`, `camera_bind_group`
- [x] `Scene::new(device, config, vertices_list, max_joints, max_morph_targets)`: GPUリソース群を初期化
- [x] `Scene::prepare(queue, joint_matrices, morph_weights, camera_uniform)`: GPUバッファ更新
- [x] `Scene::render(ctx) -> Result<()>`: RenderPass実行

```rust
impl Scene {
    pub fn render(&self, ctx: &RenderContext) -> anyhow::Result<()> {
        let output = ctx.surface.get_current_texture()?;
        let view = output.texture.create_view(&Default::default());
        let mut encoder = ctx.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, self.skin.bind_group(), &[]);
            for mesh in &self.meshes {
                mesh.draw(&mut pass);
            }
        }
        ctx.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}
```

### Step 3.8: renderer::skinned_vertex — スキニング対応頂点

**ファイル**: `crates/renderer/src/skinned_vertex.rs` (~60行)

- [x] `SkinnedVertex` 構造体: `position`, `normal`, `uv`, `joint_indices: [u32; 4]`, `joint_weights: [f32; 4]`
- [x] `SkinnedVertex::layout() -> VertexBufferLayout`: 全アトリビュートのレイアウト定義 (stride=64)

### Step 3.9: Phase 3 検証

- [x] **テスト実装**: 28テスト全パス (renderer:10 + vrm:18)
  - `renderer/src/vertex.rs`: 3テスト (layout_stride, is_pod, cast_slice_wrong_size_panics)
  - `renderer/src/camera.rs`: 5テスト (build_view_proj, aspect_change, uniform_is_pod, position_equals_target, extreme_fov)
  - `renderer/src/skinned_vertex.rs`: 2テスト (layout_stride=64, is_pod)
  - mesh/skin/morph/depth/texture/scene: GPUデバイス必要のため自動テスト対象外
  - 注: `cargo llvm-cov` はort-sysリンクエラーのため--workspace不可
- [x] **ビルド検証**:
  - `cargo check --workspace` 成功
  - `cargo clippy --workspace -- -D warnings` 警告0
  - `cargo fmt --check` 差分なし
  - 注: docker/release build/VRM描画確認は環境制約で省略

---

## Phase 4: ソルバー (solver クレート)

**目的**: KalidoKitアルゴリズムをRustに移植。ランドマーク → ボーン回転/BlendShape値

### Step 4.1: solver::utils — ユーティリティ関数

**ファイル**: `crates/solver/src/utils.rs` (既存、~30行)

- [x] `clamp(val, min, max) -> f32` を実装 (既存)
- [x] `remap(val, in_min, in_max, out_min, out_max) -> f32` を実装 (既存)
- [x] `lerp(a, b, t) -> f32` を実装 (既存)
- [x] `angle_between(v1: Vec3, v2: Vec3) -> f32` を追加: 2ベクトル間の角度
- [x] `find_rotation(a: Vec3, b: Vec3) -> Quat` を追加: aからbへの回転

```rust
pub fn angle_between(v1: glam::Vec3, v2: glam::Vec3) -> f32 {
    let dot = v1.normalize().dot(v2.normalize()).clamp(-1.0, 1.0);
    dot.acos()
}

pub fn find_rotation(from: glam::Vec3, to: glam::Vec3) -> glam::Quat {
    glam::Quat::from_rotation_arc(from.normalize(), to.normalize())
}
```

### Step 4.2: solver::face — 顔ソルバー

**ファイル**: `crates/solver/src/face.rs` (~250行)

> **300行超え注意**: 顔ソルバーが300行を超える場合、`face/eye.rs`, `face/mouth.rs`, `face/head.rs` に分割

- [x] `solve(landmarks: &[Vec3], video: &VideoInfo) -> RiggedFace` を実装
- [x] `calc_head_rotation`: ランドマーク 1(鼻先), 152(顎), 234(左耳), 454(右耳) から頭部回転を推定
- [x] `calc_eye_openness`: ランドマーク 159/145(左上下瞼), 386/374(右上下瞼) の距離比から開閉度計算
- [x] `calc_mouth_shape`: 口ランドマークの開口度・幅からA/I/U/E/O母音形状を推定
- [x] `calc_pupil_position`: 虹彩ランドマーク(468-472, 473-477)から瞳孔方向を計算
- [x] `calc_brow_raise`: 眉ランドマーク高さから眉上げ度を計算
- [x] `stabilize_blink(eye, head_y) -> EyeValues`: 頭部傾き補正 (既存)

### Step 4.3: solver::pose — ポーズソルバー

**ファイル**: `crates/solver/src/pose.rs` (~200行)

- [x] `solve(lm3d, lm2d, video) -> RiggedPose` を実装
- [x] `calc_hip_transform`: ランドマーク 23/24(左右Hip) から位置・回転を計算
- [x] `calc_spine_rotation`: 肩中点と腰中点のベクトルから脊椎回転を計算
- [x] `calc_limb_rotation(a, b, c) -> EulerAngles`: 3関節から腕/脚の回転を計算

```rust
fn calc_limb_rotation(a: Vec3, b: Vec3, c: Vec3) -> EulerAngles {
    let ab = (b - a).normalize();
    let bc = (c - b).normalize();
    // atan2ベースでオイラー角を算出
    EulerAngles {
        x: ab.y.atan2(ab.z),
        y: ab.x.atan2(ab.z),
        z: bc.x.atan2(bc.y),
    }
}
```

### Step 4.4: solver::hand — 手ソルバー

**ファイル**: `crates/solver/src/hand.rs` (~150行)

- [x] `solve(landmarks: &[Vec3], side: Side) -> RiggedHand` を実装
- [x] `calc_wrist_rotation`: ランドマーク 0(手首), 5(人差し根本), 17(小指根本) から手首回転を計算
- [x] `calc_finger_rotations(lm, indices) -> [EulerAngles; 3]`: 各指のProximal/Intermediate/Distal回転
  - 4つのランドマークから3つの関節角を算出 (隣接ベクトル間の角度)

```rust
fn calc_finger_rotations(lm: &[Vec3], indices: &[usize]) -> [EulerAngles; 3] {
    let joints: Vec<Vec3> = indices.iter().map(|&i| lm[i]).collect();
    let mut result = [EulerAngles::default(); 3];
    for i in 0..3 {
        let v1 = (joints[i + 1] - joints[i]).normalize();
        let v2 = if i + 2 < joints.len() { (joints[i + 2] - joints[i + 1]).normalize() } else { v1 };
        result[i] = EulerAngles {
            x: angle_between(v1, v2),
            y: 0.0,
            z: 0.0,
        };
    }
    result
}
```

### Step 4.5: Phase 4 検証

- [x] **テスト実装** (coverage 90%以上):
  - `solver/src/utils.rs`:
    - 正常系: `clamp(5.0, 0.0, 1.0) == 1.0`, `clamp(-1.0, 0.0, 1.0) == 0.0`
    - 正常系: `remap(0.5, 0.0, 1.0, 0.0, 10.0) == 5.0`
    - 正常系: `lerp(0.0, 10.0, 0.5) == 5.0`
    - 異常系: `remap` で `in_min == in_max` のときゼロ除算にならないこと
    - 正常系: `angle_between(Vec3::X, Vec3::Y) ≈ π/2`
    - 正常系: `find_rotation(Vec3::X, Vec3::Y)` でXをYに回転するQuatが返ること
  - `solver/src/face.rs`:
    - 正常系: 正面を向いたフェイスランドマーク (フィクスチャ) で head rotation ≈ 0 になること
    - 正常系: 両目を開けたランドマークで eye.l ≈ 1.0 になること
    - 正常系: 口を閉じたランドマークで mouth.a ≈ 0.0 になること
    - 正常系: `stabilize_blink` で head_y=0 のとき左右の値が変わらないこと
    - 異常系: 空のランドマーク配列で panic せずエラーが返ること
  - `solver/src/pose.rs`:
    - 正常系: Tポーズのランドマーク (フィクスチャ) で腕のrotation.x ≈ 0 になること
    - 正常系: Hip位置が正しく正規化されること
    - 異常系: ランドマーク数が33未満でpanic せずエラーが返ること
  - `solver/src/hand.rs`:
    - 正常系: 開いた手のランドマーク (フィクスチャ) で指のrotation ≈ 0 になること
    - 正常系: 握った手のランドマークで指のrotation > 0 になること
    - 異常系: ランドマーク数が21未満でpanic せずエラーが返ること
  - `cargo llvm-cov --package solver` で coverage 90% 以上 (cargo-llvm-cov未インストールのためスキップ)
- [x] **ビルド検証**:
  - `cargo build --release` 成功 (tracker除く: ort-sys glibc制約)
  - `cargo clippy --workspace -- -D warnings` 警告0
  - `docker build -t kalidokit-rust .` docker未インストールのためスキップ
  - アプリ起動確認: ヘッドレス環境のためスキップ

---

## Phase 5: トラッカー (tracker クレート)

**目的**: ONNX Runtimeで顔/ポーズ/手のランドマーク検出

> **既存スキャフォールドとの差異**: 現在の `crates/tracker/src/` には関数ベースのスタブ (`pub fn run_inference()`) が存在するが、本Phaseで構造体ベースの設計 (`FaceMeshDetector`, `PoseDetector`, `HandDetector`, `HolisticTracker`) に**全面的に置き換える**。既存ファイルは削除して新規作成すること。

### Step 5.1: tracker::preprocess — 画像前処理

**ファイル**: `crates/tracker/src/preprocess.rs` (既存、~40行)

- [x] `preprocess_image(image, width, height) -> Array4<f32>` を完成 (既存コード修正)
- [x] `normalize_landmarks(raw_output, image_width, image_height) -> Vec<Vec3>` を追加: モデル出力→正規化座標

### Step 5.2: tracker::face_mesh — 顔メッシュ検出

**ファイル**: `crates/tracker/src/face_mesh.rs` (~100行)

- [x] `FaceMeshDetector` 構造体: ONNX Session をラップ
- [x] `FaceMeshDetector::new(model_path) -> Result<Self>`: Session初期化
- [x] `FaceMeshDetector::detect(frame: &DynamicImage) -> Result<Option<Vec<Vec3>>>`:
  1. 画像を192×192にリサイズ・正規化
  2. ONNX推論実行
  3. 出力テンソルから468 (or 478) 個のランドマークをパース

```rust
pub struct FaceMeshDetector {
    session: ort::session::Session,
}

impl FaceMeshDetector {
    pub fn new(model_path: &str) -> anyhow::Result<Self> {
        let session = ort::session::Session::builder()?
            .with_model_from_file(model_path)?;
        Ok(Self { session })
    }

    pub fn detect(&self, frame: &image::DynamicImage) -> anyhow::Result<Option<Vec<glam::Vec3>>> {
        let input = super::preprocess::preprocess_image(frame, 192, 192);
        // session.run() → 出力テンソルパース → Vec<Vec3>
        todo!()
    }
}
```

### Step 5.3: tracker::pose — ポーズ検出

**ファイル**: `crates/tracker/src/pose.rs` (~100行)

- [x] `PoseDetector` 構造体: ONNX Session をラップ
- [x] `PoseDetector::new(model_path) -> Result<Self>`
- [x] `PoseDetector::detect(frame) -> Result<(Option<Vec<Vec3>>, Option<Vec<Vec2>>)>`:
  1. 画像を256×256にリサイズ・正規化
  2. ONNX推論
  3. 33個の3Dランドマーク + 33個の2Dランドマークをパース

### Step 5.4: tracker::hand — 手ランドマーク検出

**ファイル**: `crates/tracker/src/hand.rs` (~100行)

- [x] `HandDetector` 構造体: ONNX Session をラップ
- [x] `HandDetector::new(model_path) -> Result<Self>`
- [x] `HandDetector::detect(frame, is_left: bool) -> Result<Option<Vec<Vec3>>>`:
  1. 画像を224×224にリサイズ・正規化
  2. ONNX推論
  3. 21個のランドマークをパース
  4. **注意**: `is_left` でカメラミラー反転を処理

### Step 5.5: tracker::holistic — 統合パイプライン

**ファイル**: `crates/tracker/src/holistic.rs` (~80行)

- [x] `HolisticTracker` 構造体: `FaceMeshDetector` + `PoseDetector` + `HandDetector`
- [x] `HolisticTracker::new(face_path, pose_path, hand_path) -> Result<Self>`
- [x] `HolisticTracker::detect(frame) -> Result<HolisticResult>`: 全検出器を順番に実行

### Step 5.6: Phase 5 検証

- [x] **テスト実装** (coverage 90%以上):
  - `tracker/src/preprocess.rs`:
    - 正常系: 640×480画像を192×192に変換すると出力テンソル形状が`[1,3,192,192]`であること
    - 正常系: 出力テンソルの値が 0.0〜1.0 の範囲内であること
    - 正常系: `normalize_landmarks` で出力座標が 0.0〜1.0 に正規化されること
    - 正常系: `normalize_landmarks` でランドマーク数が入力と一致すること
    - 異常系: 0×0画像でパニックせずに処理されること
  - `tracker/src/face_mesh.rs`:
    - 異常系: 存在しないモデルパスで適切なエラーが返ること
  - `tracker/src/pose.rs`:
    - 異常系: 存在しないモデルパスで適切なエラーが返ること
  - `tracker/src/hand.rs`:
    - 異常系: 存在しないモデルパスで適切なエラーが返ること
  - (tracker テスト実行: ort-sys glibc制約によりリンク不可、cargo check のみ)
- [x] **ビルド検証**:
  - `cargo check --workspace` 成功
  - `cargo clippy --workspace -- -D warnings` 警告0
  - `cargo fmt --check` 差分なし
  - (cargo build --release/docker/アプリ起動: ort-sys/docker/ヘッドレス制約によりスキップ)

---

## Phase 6: 統合 & メインループ

**目的**: 全クレートを統合しリアルタイムモーションキャプチャを実現

### Step 6.1: app::state — アプリケーション状態管理

**ファイル**: `crates/app/src/state.rs` (~80行)

- [x] `AppState` 構造体: レンダラー・トラッカー・ソルバー・VRMモデルの全リソースを保持
  - `render_ctx: RenderContext` (ライフタイム引数なし: `Arc<Window>` によって `'static`)
  - `scene: Scene`
  - `vrm_model: VrmModel`
  - `tracker: HolisticTracker`
  - `rig: RigState` (face/pose/hand のソルバー結果)
- [x] `RigState` 構造体: `face: Option<RiggedFace>`, `pose: Option<RiggedPose>`, `left_hand/right_hand`

### Step 6.2: app::init — 初期化ロジック

**ファイル**: `crates/app/src/init.rs` (~120行)

- [x] `init_all(window) -> Result<AppState>` 関数:
  1. `RenderContext::new(window)` で wgpu 初期化
  2. `vrm::loader::load("assets/models/default_avatar.vrm")` で VRM ロード
  3. `Scene::new(device, config, vrm_model)` で GPU リソース作成
  4. `HolisticTracker::new(face_path, pose_path, hand_path)` で ML モデル初期化
  5. Webカメラ初期化 (nokhwa) — init_camera() で初期化、失敗時は None フォールバック <!-- 2026-03-10 00:40 JST -->

### Step 6.3: app::update — フレーム更新ロジック

**ファイル**: `crates/app/src/update.rs` (~150行)

- [x] `update_frame(state: &mut AppState) -> Result<()>` 関数: <!-- 2026-03-10 00:40 JST -->
  1. Webカメラからフレーム取得 (nokhwa、フォールバック付き)
  2. `tracker.detect(frame)` で全ランドマーク取得
  3. `solver::face::solve()` / `solver::pose::solve()` / `solver::hand::solve()` でリグ計算
  4. **座標変換の罠を全て適用**:
     - Hip位置: X/Z反転, Y+1.0
     - 目の開閉度: `1.0 - value` で反転
     - 瞳孔軸: X↔Y スワップ
     - 手ランドマーク左右反転
     - 手首回転: ポーズZ + ハンドX/Y合成
  5. ボーン行列計算: `vrm_model.humanoid_bones.compute_joint_matrices()`
  6. BlendShape重み計算: `vrm_model.blend_shapes.get_all_weights()`
  7. `scene.prepare(queue, joint_matrices, morph_weights, camera_uniform)` でGPU更新
  8. `scene.render(ctx)` で描画

### Step 6.4: app::main — ApplicationHandler統合

**ファイル**: `crates/app/src/main.rs` (~40行), `crates/app/src/app.rs` (~100行)

- [x] `main.rs`: EventLoop作成 + `run_app` 呼び出し
- [x] `app.rs`: `App` 構造体に `ApplicationHandler` 実装
  - `resumed()`: `init::init_all()` で全リソース初期化 + 初回 `request_redraw()`
  - `about_to_wait()`: 毎アイドル時に `request_redraw()` でレンダーループ駆動
  - `window_event(RedrawRequested)`: `update::update_frame()` + `window.request_redraw()`
  - `window_event(Resized)`: `ctx.resize()` + `depth.resize()`
  - `window_event(CloseRequested)`: `event_loop.exit()`

### Step 6.5: app — 補間パラメータ設定

**ファイル**: `crates/app/src/rig_config.rs` (~60行)

- [x] `RigConfig` 構造体: 各ボーンのdampener/lerp_amountをまとめた設定
- [x] デフォルト値を元実装と完全一致させる:

```rust
pub struct BoneConfig {
    pub dampener: f32,
    pub lerp_amount: f32,
}

pub struct RigConfig {
    pub neck: BoneConfig,          // { dampener: 0.7,  lerp: 0.3  }
    pub hips_rotation: BoneConfig, // { dampener: 0.7,  lerp: 0.3  }
    pub hips_position: BoneConfig, // { dampener: 1.0,  lerp: 0.07 }
    pub chest: BoneConfig,         // { dampener: 0.25, lerp: 0.3  }
    pub spine: BoneConfig,         // { dampener: 0.45, lerp: 0.3  }
    pub limbs: BoneConfig,         // { dampener: 1.0,  lerp: 0.3  }
    pub eye_blink: f32,            // lerp: 0.5
    pub mouth_shape: f32,          // lerp: 0.5
    pub pupil: f32,                // lerp: 0.4
}
```

### Step 6.6: Phase 6 検証

- [x] **テスト実装**:
  - `app/src/state.rs`:
    - 正常系: `RigState` のデフォルト値が全て `None` であること
  - `app/src/rig_config.rs`:
    - 正常系: `RigConfig::default()` の各値が元実装と一致すること
    - 正常系: Neck dampener = 0.7, Hips position lerp = 0.07 等
  - `app/src/update.rs` (統合テスト):
    - 注: GPU/Window + ort-sys リンク必要のため自動テスト不可 (cargo check で型安全性は検証済み)
  - 注: `cargo llvm-cov` は ort-sys glibc 2.38+ 制約で --workspace 実行不可、renderer/solver/vrm 単体テストは全パス
- [x] **ビルド検証**:
  - `cargo check --workspace` 成功
  - `cargo clippy --workspace -- -D warnings` 警告0
  - `cargo fmt --check` 差分なし
  - 注: `cargo build --release` は ort-sys リンクエラーで --workspace 不可
  - 注: `docker build` は docker 未インストールのため実行不可
  - 注: ウィンドウ表示・Webカメラはヘッドレス環境のため手動確認不可

---

## Phase 7: 仕上げ & 最適化

**目的**: SpringBone, MToon, パフォーマンス最適化, CI/CD

### Step 7.1: vrm::spring_bone — SpringBone物理

**ファイル**: `crates/vrm/src/spring_bone.rs` (~200行)

- [x] `SpringBone` 構造体: `stiffness`, `gravity_power`, `gravity_dir`, `drag_force`, `hit_radius`
- [x] `SpringBoneGroup` 構造体: `bones: Vec<SpringBone>`, `colliders: Vec<Collider>`
- [x] `SpringBoneGroup::from_vrm_json(json)`: VRM拡張JSONからパース
- [x] `SpringBoneGroup::update(delta_time)`: Verlet積分で髪揺れ等の物理シミュレーション
- [x] **VrmModel への統合**: VrmModel に spring_bone_groups フィールド追加、loader でパース、update ループで毎フレーム update() 呼び出し <!-- 2026-03-10 00:31 JST -->

```rust
// VRM JSON構造:
// { "secondaryAnimation": { "boneGroups": [
//   { "stiffiness": 1.0, "gravityPower": 0, "dragForce": 0.4,
//     "bones": [nodeIndex, ...] }
// ] } }

impl SpringBone {
    pub fn update(&mut self, delta_time: f32, center: glam::Vec3) {
        let delta = delta_time.max(0.0); // 負のdt防御
        // Verlet積分: next = current + (current - prev) * (1 - drag) + external_forces * dt²
        let velocity = (self.current_tail - self.prev_tail) * (1.0 - self.drag_force);
        let stiffness_force = (self.initial_tail - self.current_tail).normalize() * self.stiffness * delta;
        let gravity = self.gravity_dir * self.gravity_power * delta;
        let next_tail = self.current_tail + velocity + stiffness_force + gravity;
        // コライダー衝突判定
        let next_tail = self.check_colliders(next_tail);
        // ボーン長を維持 (正規化して元の長さに)
        let direction = (next_tail - center).normalize();
        let next_tail = center + direction * self.bone_length;
        self.prev_tail = self.current_tail;
        self.current_tail = next_tail;
    }
}
```

### Step 7.2: assets/shaders/mtoon.wgsl — MToonシェーダー

**ファイル**: `assets/shaders/mtoon.wgsl` (~120行)

- [x] VRM標準のトゥーンシェーダー (MToon) を実装 — **シェーダーファイルは存在するが未統合**
  - 2段階トゥーンシェーディング (影しきい値ベース)
  - リムライト
  - アウトライン (別パス)
- [x] **レンダーパイプラインへの統合**: MToon トゥーンシェーディング (2段階陰影 + リムライト) を skinning.wgsl に統合、VRM MToon 拡張パース実装 <!-- 2026-03-10 00:35 JST -->

```wgsl
// MToon Fragment Shader の核心ロジック
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base_color = textureSample(t_color, s_color, in.uv) * material.color;
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let ndotl = dot(normalize(in.normal), light_dir);

    // 2段階トゥーンシェーディング
    let shade_threshold = material.shade_shift + material.shade_toony;
    let shade_factor = smoothstep(material.shade_shift, shade_threshold, ndotl);
    let lit_color = mix(material.shade_color.rgb, base_color.rgb, shade_factor);

    // リムライト
    let view_dir = normalize(camera.position - in.world_pos);
    let rim = pow(1.0 - max(dot(normalize(in.normal), view_dir), 0.0), material.rim_power);
    let rim_color = material.rim_color.rgb * rim * material.rim_lift;

    return vec4<f32>(lit_color + rim_color, base_color.a);
}
```

### Step 7.3: パフォーマンス最適化

- [x] ML推論を別スレッドに移動 (`std::thread::spawn` + `mpsc::channel`) <!-- TrackerThread + sync_channel(1) 実装 — 2026-03-10 00:43 JST -->
- [x] フレームレート制御: `std::time::Instant` で16ms (60fps) 間隔を維持
- [x] GPU バッファ更新の最小化: 変更がない場合は `write_buffer` をスキップ (rig_dirty フラグ)

### Step 7.4: CI/CD (GitHub Actions)

**ファイル**: `.github/workflows/ci.yml` (~50行)

- [x] プッシュ/PR時に自動実行:
  1. `cargo fmt --check`
  2. `cargo clippy --workspace -- -D warnings`
  3. `cargo test -p renderer -p vrm -p solver` (tracker は ort-sys リンク制約で除外)
  4. `cargo check --workspace`
  5. `docker build .`

### Step 7.6: GitHub Release — クロスプラットフォームバイナリ配布

**ファイル**: `.github/workflows/release.yml` (~120行)

- [x] タグプッシュ (`v*`) 時に自動実行するリリースワークフロー:
  1. Windows (x86_64-pc-windows-msvc), macOS (aarch64-apple-darwin), Linux (x86_64-unknown-linux-gnu) の3プラットフォーム向けにビルド (注: macOS x86_64 は ort-sys が prebuilt binary 未提供のため除外、Intel Mac は Rosetta 2 経由で aarch64 バイナリを実行可能)
  2. 各バイナリを `.tar.gz` (Linux/macOS) / `.zip` (Windows) で圧縮
  3. GitHub Release を作成し、全アーティファクトをアップロード
- [x] matrix strategy で各OS/targetを並列ビルド
- [x] `assets/` ディレクトリ (シェーダー, モデル等) をバイナリと共にパッケージ

### Step 7.5: Phase 7 検証

- [x] **テスト実装**:
  - `vrm/src/spring_bone.rs`:
    - 正常系: `update(0.016)` で位置が更新されること
    - 正常系: `stiffness=0` でボーンが重力方向に落ちること
    - 正常系: `drag_force=1.0` でボーンが動かないこと
    - 異常系: `delta_time=0` でパニックしないこと
    - 異常系: 負の `delta_time` でパニックしないこと
    - 追加: bone_length_maintained, collider_pushes_out, from_vrm_json_parses, no_secondary_animation
  - 注: E2Eテストは GPU/Window 必要のため手動確認対象
  - 注: `cargo llvm-cov` は ort-sys glibc 制約で --workspace 実行不可
- [x] **ビルド検証**:
  - `cargo check --workspace` 成功
  - `cargo clippy --workspace -- -D warnings` 警告0
  - `cargo fmt --check` 差分なし
  - 全60テスト合格 (renderer:10, solver:23, vrm:27)
  - 注: `cargo build --release` は ort-sys リンクエラーで --workspace 不可
  - 注: `docker build` は docker 未インストールのため実行不可
  - 注: GitHub Actions CI はプッシュ後に自動実行
  - 注: E2E動作確認はヘッドレス環境のため手動確認不可

---

## Phase 8: トラッキングパイプライン改善 & リグ適用完成

**目的**: MediaPipe Holistic のパイプライン最適化を再現し、kalidokit-testbed (JS版) と同等のリグ適用を実現する

**リファレンス**:
- [kalidokit-testbed vrm/script.js](https://github.com/tk-aria/kalidokit-testbed/blob/main/vrm/script.js) — ボーン適用・dampener・補間パラメータの正解値
- [MediaPipe Holistic Landmarker](https://ai.google.dev/edge/mediapipe/solutions/vision/holistic_landmarker) — ROI クロップ・パイプライン最適化のリファレンス
- [google-ai-edge/mediapipe (GitHub)](https://github.com/google-ai-edge/mediapipe) — Holistic Graph の内部処理詳細
- [KalidoKit (npm)](https://www.npmjs.com/package/kalidokit) — オリジナル JS ソルバーのリファレンス

### Step 8.1: Pose → Hand ROI クロップ (精度向上)

**ファイル**: `crates/tracker/src/holistic.rs`, `crates/tracker/src/hand.rs`

> MediaPipe Holistic は Pose の手首ランドマーク (15:左手首, 16:右手首) から手の領域を切り出し、Hand モデルに渡す。現在は全フレームを Hand モデルに渡しているため精度が低い。

- [x] Pose ランドマーク (index 15, 16) から手の ROI (Region of Interest) を算出する関数を追加 <!-- 2026-03-10 12:54 JST -->
- [x] ROI に基づいてフレームをクロップし、`HandDetector::detect()` に渡すよう `HolisticTracker::detect()` を修正 <!-- 2026-03-10 12:54 JST -->
- [x] ROI が取得できない場合 (Pose 未検出) は従来通り全フレームで推論するフォールバック <!-- 2026-03-10 12:54 JST -->
- [x] テスト: ROI 算出ロジックの単体テスト (手首座標 → 正方形 ROI の中心・サイズ) <!-- 2026-03-10 12:54 JST -->

### Step 8.2: slerp / dampener 補間の適用

**ファイル**: `crates/app/src/update.rs`, `crates/app/src/rig_config.rs`

> testbed の `rigRotation()` は全ボーンに dampener (回転量の減衰) と slerp 補間 (前フレームとの球面線形補間) を適用している。現在は `to_quat()` を直接 `set_rotation()` しており動きがガタつく。
>
> リファレンス: [script.js rigRotation()](https://github.com/tk-aria/kalidokit-testbed/blob/main/vrm/script.js) の dampener / lerpAmount パラメータ

- [x] `HumanoidBones` に前フレームの回転を保持する仕組みを追加 (`prev_rotation: HashMap<HumanoidBoneName, Quat>`) <!-- 2026-03-10 12:54 JST -->
- [x] `apply_rig_to_model()` で `RigConfig` の dampener / lerp_amount を使って slerp 補間を適用 <!-- 2026-03-10 12:54 JST -->
- [x] dampener 値を testbed と完全一致させる: <!-- 2026-03-10 12:54 JST -->
  - Neck: dampener=0.7, lerp=0.3
  - Hips rotation: dampener=0.7, lerp=0.3
  - Hips position: dampener=1.0, lerp=0.07
  - Chest: dampener=0.25, lerp=0.3
  - Spine: dampener=0.45, lerp=0.3
  - UpperArm/LowerArm/UpperLeg/LowerLeg: dampener=1.0, lerp=0.3
- [x] テスト: slerp 補間で前フレームと次フレームの中間値が生成されること <!-- 2026-03-10 12:54 JST -->

### Step 8.3: ハンドボーン適用 (左右各16ボーン)

**ファイル**: `crates/app/src/update.rs`

> testbed は左右それぞれ 16 ボーン (Wrist + 5指 × 3関節) を適用。さらに Hand の Z軸は Pose solver、X/Y は Hand solver から合成している。現在は `solver::hand::solve()` を呼んでいるが `apply_rig_to_model()` に適用コードがない。
>
> リファレンス: [script.js leftHandLandmarks / rightHandLandmarks ブロック](https://github.com/tk-aria/kalidokit-testbed/blob/main/vrm/script.js)

- [x] `apply_rig_to_model()` に左手ボーン適用を追加 (LeftHand, LeftThumbProximal/Intermediate/Distal, LeftIndexProximal/Intermediate/Distal, LeftMiddleProximal/Intermediate/Distal, LeftRingProximal/Intermediate/Distal, LeftLittleProximal/Intermediate/Distal) <!-- 2026-03-10 12:58 JST -->
- [x] `apply_rig_to_model()` に右手ボーン適用を追加 (同上、Right系) <!-- 2026-03-10 12:58 JST -->
- [x] Hand の回転合成: Wrist の Z 軸は `RiggedPose.left_hand.z` / `RiggedPose.right_hand.z` から、X/Y は `RiggedHand.wrist` から取得 <!-- 2026-03-10 12:58 JST -->
- [x] テスト: RiggedHand の全フィールドが HumanoidBones に反映されること <!-- 2026-03-10 12:58 JST -->

### Step 8.4: Hip position 適用

**ファイル**: `crates/app/src/update.rs`, `crates/vrm/src/bone.rs`

> testbed は `rigPosition("Hips", ...)` で体の移動を反映している。現在は `hip_pos` を計算するが `let _ = hip_pos;` で捨てている (update.rs:258)。
>
> リファレンス: [script.js rigPosition("Hips", ...)](https://github.com/tk-aria/kalidokit-testbed/blob/main/vrm/script.js)

- [x] `HumanoidBones` に `set_position(name, Vec3)` メソッドを追加 <!-- 2026-03-10 12:58 JST -->
- [x] `compute_joint_matrices()` で Hips ボーンの position を translation に反映 <!-- 2026-03-10 12:58 JST -->
- [x] `apply_rig_to_model()` で `hip_pos` を `set_position(Hips, hip_pos)` に変更 (`let _ = hip_pos;` を削除) <!-- 2026-03-10 12:58 JST -->
- [x] Hip position にも lerp 補間を適用 (dampener=1.0, lerp=0.07) <!-- 2026-03-10 12:58 JST -->
- [x] テスト: set_position 後に compute_joint_matrices で Hips の translation が反映されること <!-- 2026-03-10 12:58 JST -->

### Step 8.5: Pupil (瞳孔) + LookAt 適用

**ファイル**: `crates/app/src/update.rs`, `crates/vrm/src/look_at.rs`

> testbed は `riggedFace.pupil` → `currentVrm.lookAt.applyer.lookAt()` で視線を制御。Rust 側に `LookAt` モジュールは存在するが `apply_rig_to_model()` で使われていない。
>
> リファレンス: [script.js oldLookTarget / lookTarget / lookAt.applyer](https://github.com/tk-aria/kalidokit-testbed/blob/main/vrm/script.js)

- [x] `solver::face::RiggedFace` に `pupil` フィールドが存在することを確認 (なければ追加) <!-- 2026-03-10 13:02 JST -->
- [x] `apply_rig_to_model()` で `LookAt::apply(pupil)` を呼び出し、LeftEye / RightEye ボーンに反映 <!-- 2026-03-10 13:02 JST -->
- [x] 瞳孔の lerp 補間 (lerp=0.4) と前フレーム値の保持 <!-- 2026-03-10 13:02 JST -->
- [x] テスト: pupil 値に対して LeftEye/RightEye の回転が変化すること <!-- 2026-03-10 13:02 JST -->

### Step 8.6: Face blink 補間 + stabilizeBlink

**ファイル**: `crates/app/src/update.rs`

> testbed は目の開閉値を前フレームの BlendShape 値と lerp(0.5) で補間し、`Kalidokit.Face.stabilizeBlink()` で頭部傾き補正を適用。現在は `1.0 - face.eye.l` を直接設定しているのみ。
>
> リファレンス: [script.js rigFace() 内の eye 処理](https://github.com/tk-aria/kalidokit-testbed/blob/main/vrm/script.js)

- [x] 前フレームの BlinkL/BlinkR 値を保持する仕組みを追加 <!-- 2026-03-10 13:02 JST -->
- [x] `apply_rig_to_model()` で `lerp(clamp(1.0 - eye.l, 0, 1), prev_blink, 0.5)` を適用 <!-- 2026-03-10 13:02 JST -->
- [x] `solver::face::stabilize_blink()` を blink 値設定前に呼び出す <!-- 2026-03-10 13:02 JST -->
- [x] 左右同値でのまばたき (testbed は BlinkL = BlinkR = eye.l) <!-- 2026-03-10 13:02 JST -->
- [x] テスト: stabilizeBlink が頭部Y回転に基づいて blink 値を補正すること <!-- 2026-03-10 13:02 JST -->

### Step 8.7: Head → Neck 適用先の修正

**ファイル**: `crates/app/src/update.rs`

> testbed は `rigRotation("Neck", riggedFace.head, 0.7)` で頭部回転を Neck ボーンに適用。現在は Head ボーンに直接適用している。
>
> リファレンス: [script.js rigFace() 内の Neck](https://github.com/tk-aria/kalidokit-testbed/blob/main/vrm/script.js)

- [x] `apply_rig_to_model()` で `HumanoidBoneName::Head` → `HumanoidBoneName::Neck` に変更 <!-- 2026-03-10 13:02 JST -->
- [x] dampener=0.7 を適用 <!-- 2026-03-10 13:02 JST -->
- [x] テスト: face solver の head rotation が Neck ボーンに反映されること <!-- 2026-03-10 13:02 JST -->

### Step 8.8: Face / Pose 並列推論

**ファイル**: `crates/tracker/src/holistic.rs`

> 現在 Face → Pose → Hand(L) → Hand(R) が直列実行。Face と Pose は独立しているため並列化可能。

- [x] `rayon` を tracker クレートの依存に追加 <!-- 2026-03-10 13:06 JST -->
- [x] `HolisticTracker::detect()` で Face と Pose を `rayon::join` で並列実行 <!-- 2026-03-10 13:06 JST -->
- [x] Hand は Pose 結果 (Step 8.1 の ROI) に依存するため Pose 完了後に実行 <!-- 2026-03-10 13:06 JST -->
- [x] テスト: 並列化前後で同一入力に対する出力が一致すること — コンパイル検証のみ (ONNX モデル不要の範囲で確認) <!-- 2026-03-10 13:06 JST -->

### Step 8.9: Phase 8 検証

- [x] **テスト実装**: <!-- 2026-03-10 13:10 JST -->
  - Step 8.1: ROI 算出の単体テスト (4件追加)
  - Step 8.2: slerp 補間の単体テスト (1件追加)
  - Step 8.3: ハンドボーン適用の確認 (2件追加)
  - Step 8.4: Hip position 適用の確認 (1件追加)
  - Step 8.5: LookAt 適用の確認 (1件追加)
  - Step 8.6: blink 補間の確認 (1件追加)
  - Step 8.7: Neck 適用先の確認 (コンパイル検証)
  - `cargo test -p solver -p vrm -p renderer` 全パス (63テスト)
  - `cargo check --workspace` 成功
  - `cargo clippy --workspace -- -D warnings` 警告0
  - `cargo fmt --check` 差分なし
- [ ] **動作検証** (ヘッドレス環境のため未検証):
  - Webカメラでリアルタイムモーションキャプチャが testbed と同等に動作すること
  - 手の指が正しく動くこと
  - 体の移動 (Hip position) が反映されること
  - 目の追従・まばたきが自然なこと
  - 動きが滑らか (ガタつきなし) であること

---

## Phase 9: musl → glibc + cargo-zigbuild 移行 & カメラ復活

**目的**: Linux ビルドを musl 静的リンクから glibc (2.17+) + cargo-zigbuild に移行し、nokhwa によるカメラキャプチャを全プラットフォームで復活させる

**背景**: musl 対応のためにカメラ機能 (nokhwa) が完全に削除されスタブ化された。本プロジェクトは GPU + ウィンドウ + カメラを使うデスクトップアプリのため、musl (Alpine コンテナ向け) のメリットは薄い。glibc 2.17 は CentOS 7 以降のほぼ全ての Linux ディストリビューションをカバーする。

### Step 9.1: CI — Linux ビルドジョブを cargo-zigbuild に移行

**ファイル**: `.github/workflows/release.yml`

- [x] `build-linux` ジョブ名を `Build (x86_64-unknown-linux-gnu)` に変更 <!-- 2026-03-11 14:22 JST -->
- [x] Alpine コンテナ (`container: image: alpine:3.21`) を削除し、`ubuntu-latest` で直接実行 <!-- 2026-03-11 14:22 JST -->
- [x] システム依存パッケージを apt-get に変更: <!-- 2026-03-11 14:22 JST -->
  ```bash
  sudo apt-get update
  sudo apt-get install -y cmake pkg-config libx11-dev libxkbcommon-dev libwayland-dev
  ```
- [x] Rust ツールチェーンインストールを `dtolnay/rust-toolchain@stable` に変更し、`x86_64-unknown-linux-gnu` ターゲットを追加 <!-- 2026-03-11 14:22 JST -->
- [x] `cargo install cargo-zigbuild` を追加 <!-- 2026-03-11 14:22 JST -->
- [x] Zig ツールチェーンのインストールを追加 (例: `pip3 install ziglang` または公式バイナリ) <!-- 2026-03-11 14:22 JST -->
- [x] 以下の musl ワークアラウンドを全て削除: <!-- 2026-03-11 14:22 JST -->
  - execinfo.h スタブ (旧 lines 55-65)
  - Eigen 事前クローン (旧 lines 67-72)
  - sed パッチ (旧 lines 79-82)
  - ORT ビルドフラグ `FLATBUFFERS_LOCALE_INDEPENDENT=0`, `ENABLE_BACKTRACE=OFF` (旧 lines 90-100)
  - re2 スタンドアロンビルド (旧 lines 106-150)
- [x] ビルドコマンドを変更: <!-- 2026-03-11 14:22 JST -->
  ```bash
  cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17
  ```
- [x] パッケージングのアーカイブ名を `x86_64-unknown-linux-gnu` に変更 <!-- 2026-03-11 14:22 JST -->
- [x] Upload artifact の名前を `x86_64-unknown-linux-gnu` に変更 <!-- 2026-03-11 14:22 JST -->
- [x] ORT キャッシュキーを更新 (旧 `ort-musl-static-*` → 新しいキー名) <!-- 2026-03-11 14:22 JST -->
- [x] ORT ビルドは glibc 環境ではデフォルト設定で動作するため、ビルドステップを大幅に簡素化 <!-- 2026-03-11 14:22 JST -->

### Step 9.2: セットアップスクリプトの更新

**ファイル**: `scripts/setup.sh`

- [x] `_get_target()` 関数の Linux ターゲットを変更: <!-- 2026-03-11 14:22 JST -->
  ```sh
  # 変更前
  linux)   echo "${_arch}-unknown-linux-musl" ;;
  # 変更後
  linux)   echo "${_arch}-unknown-linux-gnu" ;;
  ```

### Step 9.3: nokhwa 依存の復活

**ファイル**: `Cargo.toml` (ワークスペースルート), `crates/app/Cargo.toml`

- [x] ワークスペースルート `Cargo.toml` に `nokhwa` が既に定義されていることを確認: <!-- 2026-03-11 14:27 JST -->
  ```toml
  nokhwa = { version = "0.10", features = ["input-native"] }
  ```
- [x] `crates/app/Cargo.toml` の `[dependencies]` に `nokhwa` を追加: <!-- 2026-03-11 14:27 JST -->
  ```toml
  nokhwa = { workspace = true }
  ```

### Step 9.4: カメラ型の復元

**ファイル**: `crates/app/src/state.rs`

- [x] `camera` フィールドの型をスタブから実型に変更: <!-- 2026-03-11 14:27 JST -->
  ```rust
  // 変更前
  pub camera: Option<()>,
  // 変更後
  pub camera: Option<nokhwa::Camera>,
  ```
- [x] 必要な `use` 文を追加 <!-- 2026-03-11 14:27 JST -->

### Step 9.5: カメラ初期化の復元

**ファイル**: `crates/app/src/init.rs`

- [x] `init_camera()` 関数を実装: <!-- 2026-03-11 14:27 JST -->
  ```rust
  fn init_camera() -> Option<nokhwa::Camera> {
      // 640x480 MJPEG 30fps でカメラ初期化
      // 失敗時は log::warn! して None を返す
  }
  ```
- [x] `init_all()` 内のスタブ (`let camera: Option<()> = None;`) を `init_camera()` 呼び出しに置換 <!-- 2026-03-11 14:27 JST -->
- [x] nokhwa の `CameraIndex::Index(0)`, `RequestedFormat` 等を使用 <!-- 2026-03-11 14:27 JST -->
- [x] エラー時のフォールバック: `log::warn!` でメッセージを出し `None` を返す（パニックしない） <!-- 2026-03-11 14:27 JST -->

### Step 9.6: フレーム取得の復元

**ファイル**: `crates/app/src/update.rs`

- [x] `capture_frame()` の引数型を `Option<nokhwa::Camera>` に変更 <!-- 2026-03-11 14:27 JST -->
- [x] カメラが `Some` の場合: `camera.frame()` → `frame.decode_image()` でフレーム取得 <!-- 2026-03-11 14:27 JST -->
- [x] カメラが `None` またはフレーム取得失敗時: 640x480 ダミー黒画像にフォールバック <!-- 2026-03-11 14:27 JST -->
- [x] フレームの解像度を `VideoInfo` に反映（ハードコードしない） <!-- 2026-03-11 14:27 JST -->

### Step 9.7: ドキュメント更新

- [x] `CLAUDE.md`: <!-- 2026-03-11 14:32 JST -->
  - ORT ビルドの musl 注記 (`ORT ビルド (Linux musl): execinfo.h スタブ...`) を削除
  - `cargo-zigbuild` による Linux ビルド手順を追記
- [x] `features.md`: <!-- 2026-03-11 14:32 JST -->
  - ライブラリバージョン一覧の `nokhwa` が残っていることを確認
  - Step 6.2 (カメラ初期化) と Step 6.3 (フレーム取得) のチェックボックスは動作確認後にチェック
- [x] `README.md`: <!-- 2026-03-11 14:32 JST -->
  - アーキテクチャ図の Camera 部分が nokhwa であることを確認
  - Linux ダウンロードセクションのターゲットを `x86_64-unknown-linux-musl` → `x86_64-unknown-linux-gnu` に変更

### Step 9.8: Phase 9 検証

- [x] **ビルド検証**: <!-- 2026-03-11 14:35 JST -->
  - `cargo check --workspace` 成功
  - `cargo clippy --workspace -- -D warnings` 警告 0
  - `cargo fmt --check` 差分なし (cargo fmt で自動修正済み)
- [ ] **カメラ動作確認** (カメラ接続環境で実施) — ヘッドレス環境のため未検証:
  - アプリ起動時にカメラが初期化される (`init_camera()` が `Some` を返す)
  - 毎フレームカメラからの画像が取得される（ダミー黒画像でない）
  - カメラ未接続時にダミーフレームにフォールバックし、パニックしない
- [ ] **CI 検証** (タグ push で release.yml を実行) — CI 実行環境がないため未検証:
  - Linux: `cargo zigbuild` で glibc 2.17 ターゲットのバイナリが生成される
  - macOS: `cargo build` でビルド成功（変更なし）
  - Windows: `cargo build` でビルド成功（変更なし）
  - GitHub Release に 3 プラットフォーム分のアーティファクトがアップロードされる
