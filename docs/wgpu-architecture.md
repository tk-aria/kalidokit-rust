# KalidoKit Rust - wgpu直接アーキテクチャ設計書

## 1. 方針: Bevy参考 × wgpu直接

Bevyのレンダリングアーキテクチャ (Render Graph, Extract/Prepare/Queue/Render ステージ) を
参考にしつつ、VRMモーションキャプチャに必要な最小限のレンダラーをwgpu上に構築する。

### Bevyから借用する設計パターン

| Bevyの概念 | 本プロジェクトでの適用 |
|-----------|---------------------|
| Render Graph | 簡易版: 単一パスの Forward Rendering |
| Extract Stage | カメラフレーム→テクスチャ転送 |
| Prepare Stage | ボーン行列・BlendShape重み→GPU Buffer更新 |
| Render Stage | glTFメッシュ描画 + Skinning + MorphTarget |
| ECS World | 不使用 → 構造体ベースのシンプルなシーン管理 |

### Bevyから借用しない部分 (軽量化)

- ECS (Entity Component System) → 不要、直接構造体管理
- Plugin System → 不要、モジュール分割で十分
- Asset Pipeline → 不要、起動時に一度VRMをロード
- Render Sub Graph → 不要、単一シーン
- UI System → 不要

---

## 2. アーキテクチャ全体図

```
┌──────────────────────────────────────────────────────────┐
│                    Application                           │
│                                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │ Camera   │  │ Tracker  │  │ Solver   │  │Renderer │ │
│  │ (nokhwa) │  │ (ort)    │  │          │  │ (wgpu)  │ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬────┘ │
│       │              │             │              │      │
│       ▼              ▼             ▼              ▼      │
│  ┌─────────────────────────────────────────────────────┐ │
│  │                  Main Loop (winit)                  │ │
│  │                                                     │ │
│  │  1. Poll Events (winit)                             │ │
│  │  2. Capture Frame (nokhwa)                          │ │
│  │  3. ML Inference (ort)                              │ │
│  │  4. Solve Rig (solver)                              │ │
│  │  5. Update GPU Buffers (wgpu)  ← Bevy Prepare相当  │ │
│  │  6. Render Pass (wgpu)         ← Bevy Render相当   │ │
│  │  7. Present (wgpu surface)                          │ │
│  └─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

---

## 3. レンダリングパイプライン (Bevy参考)

### Bevyの4ステージをシンプル化

```
Bevy:     Extract → Prepare → Queue → Render
本PJ:              Prepare → Render (2ステージ)

Prepare:
  ├─ ボーン行列 (joint_matrices) を計算
  │   → Quaternion slerp補間後の最終行列
  │   → GPU Uniform Buffer に書き込み
  │
  ├─ MorphTarget重み (blend_weights) を計算
  │   → BlendShape値をlerp補間
  │   → GPU Storage Buffer に書き込み
  │
  └─ カメラ行列 (view_proj) を更新
      → GPU Uniform Buffer に書き込み

Render:
  └─ Forward Render Pass
      ├─ Vertex Shader: Skinning + MorphTarget
      ├─ Fragment Shader: PBR (簡易版) or MToon
      └─ Draw Call per mesh primitive
```

---

## 4. GPU パイプライン構成

### 4.1 Vertex Shader (Skinning + MorphTarget)

```wgsl
// shader.wgsl - Bevyのskinning.wgslを参考

struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
};

struct JointMatrices {
    matrices: array<mat4x4<f32>, 256>,  // 最大256ボーン
};

struct MorphWeights {
    weights: array<f32, 64>,  // 最大64 MorphTarget
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<storage, read> joints: JointMatrices;
@group(2) @binding(0) var<storage, read> morph_weights: MorphWeights;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) joint_indices: vec4<u32>,   // スキニング: どのボーンに影響されるか
    @location(4) joint_weights: vec4<f32>,   // スキニング: 各ボーンの影響度
    // MorphTarget差分はStorage Bufferから読む
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    // 1. MorphTarget適用 (BlendShape)
    var position = input.position;
    // morph_targets から差分を加算 (実装時に展開)

    // 2. Skinning (ボーンアニメーション)
    // Bevyと同様: 最大4ボーンの加重平均
    let skin_matrix =
        joints.matrices[input.joint_indices.x] * input.joint_weights.x +
        joints.matrices[input.joint_indices.y] * input.joint_weights.y +
        joints.matrices[input.joint_indices.z] * input.joint_weights.z +
        joints.matrices[input.joint_indices.w] * input.joint_weights.w;

    let world_position = camera.model * skin_matrix * vec4<f32>(position, 1.0);

    var output: VertexOutput;
    output.clip_position = camera.view_proj * world_position;
    output.world_normal = (camera.model * skin_matrix * vec4<f32>(input.normal, 0.0)).xyz;
    output.uv = input.uv;
    return output;
}
```

### 4.2 Fragment Shader (簡易PBR)

```wgsl
@group(3) @binding(0) var base_color_texture: texture_2d<f32>;
@group(3) @binding(1) var base_color_sampler: sampler;

struct Light {
    direction: vec3<f32>,
    color: vec3<f32>,
};

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let base_color = textureSample(base_color_texture, base_color_sampler, input.uv);
    let normal = normalize(input.world_normal);
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));

    // Lambert diffuse
    let ndotl = max(dot(normal, light_dir), 0.0);
    let ambient = 0.3;
    let diffuse = ndotl * 0.7;

    let color = base_color.rgb * (ambient + diffuse);
    return vec4<f32>(color, base_color.a);
}
```

---

## 5. ディレクトリ構成 (wgpu版)

```
kalidokit-rust/
├── Cargo.toml
├── assets/
│   ├── models/
│   │   └── default_avatar.vrm
│   ├── ml/
│   │   ├── face_landmark.onnx
│   │   ├── pose_landmark.onnx
│   │   └── hand_landmark.onnx
│   └── shaders/
│       ├── skinning.wgsl          # Vertex: Skinning + MorphTarget
│       ├── pbr.wgsl               # Fragment: 簡易PBR
│       └── mtoon.wgsl             # Fragment: MToon (VRM用トゥーン)
│
├── crates/
│   ├── app/                       # メインアプリケーション (bin)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs            # winit + wgpu 初期化・メインループ
│   │       └── app.rs             # Appステート管理
│   │
│   ├── renderer/                  # wgpuレンダラー (lib) ← 新規
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── context.rs         # wgpu Device/Queue/Surface 管理
│   │       ├── pipeline.rs        # RenderPipeline 構築
│   │       ├── camera.rs          # カメラ行列 (View/Projection)
│   │       ├── scene.rs           # シーン管理 (メッシュ・ライト)
│   │       ├── mesh.rs            # メッシュデータ (Vertex/Index Buffer)
│   │       ├── skin.rs            # スキニング (ボーン行列管理)
│   │       ├── morph.rs           # MorphTarget (BlendShape管理)
│   │       ├── texture.rs         # テクスチャ管理
│   │       └── uniform.rs         # Uniform/Storage Buffer管理
│   │
│   ├── vrm/                       # VRMローダー (lib) ← 新規
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── loader.rs          # VRM (glTF+拡張) ファイル読み込み
│   │       ├── model.rs           # VRMモデルデータ構造
│   │       ├── bone.rs            # ヒューマノイドボーンマッピング
│   │       ├── blendshape.rs      # BlendShapeプリセット管理
│   │       ├── spring_bone.rs     # SpringBone物理 (髪揺れ等)
│   │       └── look_at.rs         # LookAt (視線制御)
│   │
│   ├── solver/                    # ソルバー (変更なし)
│   │   └── src/ ...
│   │
│   └── tracker/                   # トラッカー (変更なし)
│       └── src/ ...
│
└── docs/
    ├── design.md                  # 元設計書
    └── wgpu-architecture.md       # 本ドキュメント
```

---

## 6. シーケンス図 (wgpu版)

```
┌──────┐ ┌──────┐ ┌───────┐ ┌──────┐ ┌────────┐ ┌──────────┐ ┌──────┐
│winit │ │nokhwa│ │  ort  │ │solver│ │renderer│ │   vrm    │ │ wgpu │
│event │ │camera│ │(ONNX) │ │      │ │(custom)│ │ (loader) │ │(GPU) │
└──┬───┘ └──┬───┘ └──┬────┘ └──┬───┘ └───┬────┘ └────┬─────┘ └──┬───┘
   │        │        │         │         │           │          │
   │ [1] 起動・初期化 │         │         │           │          │
   │────────│────────│─────────│─────────│───────────│─────────▶│
   │        │        │         │         │           │ Device   │
   │        │        │         │         │           │ Queue    │
   │        │        │         │         │           │ Surface  │
   │        │        │         │         │           │ Pipeline │
   │        │        │         │         │           │          │
   │        │        │         │         │ [2] VRM   │          │
   │        │        │         │         │ ロード    │          │
   │        │        │         │         │──────────▶│          │
   │        │        │         │         │ meshes    │          │
   │        │        │         │         │ joints    │──────────▶│
   │        │        │         │         │ morphs    │ buffers  │
   │        │        │         │         │◀──────────│          │
   │        │        │         │         │           │          │
   ║════════║════ メインループ ═║═════════║═══════════║══════════║
   │        │        │         │         │           │          │
   │ [3]    │        │         │         │           │          │
   │ Event  │        │         │         │           │          │
   │ Poll   │        │         │         │           │          │
   │───┐    │        │         │         │           │          │
   │   │    │        │         │         │           │          │
   │◀──┘    │        │         │         │           │          │
   │        │        │         │         │           │          │
   │ [4] フレーム取得  │         │         │           │          │
   │───────▶│        │         │         │           │          │
   │  RGB   │        │         │         │           │          │
   │◀───────│        │         │         │           │          │
   │        │        │         │         │           │          │
   │ [5] ML推論       │         │         │           │          │
   │────────────────▶│         │         │           │          │
   │  landmarks      │         │         │           │          │
   │◀────────────────│         │         │           │          │
   │        │        │         │         │           │          │
   │ [6] ソルバー     │         │         │           │          │
   │───────────────────────── ▶│         │           │          │
   │  RiggedFace/Pose/Hand    │         │           │          │
   │◀─────────────────────────│         │           │          │
   │        │        │         │         │           │          │
   │ [7] Prepare (GPU Buffer更新)       │           │          │
   │────────────────────────────────────▶│           │          │
   │        │        │         │  joint_matrices     │          │
   │        │        │         │  morph_weights      │──────────▶│
   │        │        │         │  camera_uniform     │ write_buf│
   │        │        │         │         │           │          │
   │ [8] Render Pass │         │         │           │          │
   │────────────────────────────────────▶│           │          │
   │        │        │         │         │──────────────────────▶│
   │        │        │         │         │ begin_render_pass    │
   │        │        │         │         │ set_pipeline         │
   │        │        │         │         │ set_bind_group       │
   │        │        │         │         │ draw_indexed         │
   │        │        │         │         │ end_render_pass      │
   │        │        │         │         │◀─────────────────────│
   │        │        │         │         │           │          │
   │ [9] Present     │         │         │           │          │
   │────────────────────────────────────────────────────────────▶│
   │        │        │         │         │           │ present()│
   ║════════║════════║═════════║═════════║═══════════║══════════║
```

---

## 7. クレート別 入出力 & サンプルコード

### 7.1 `renderer` クレート

#### 入出力

| モジュール | 入力 | 出力 |
|-----------|------|------|
| `context::new` | `winit::Window` | `RenderContext { device, queue, surface }` |
| `pipeline::create` | `&Device, ShaderSource` | `RenderPipeline` |
| `skin::update` | `&[Mat4]` (ボーン行列) | GPU Buffer 書き込み |
| `morph::update` | `&[f32]` (BlendShape重み) | GPU Buffer 書き込み |
| `scene::render` | `&RenderContext, &Scene` | フレーム描画 |

#### サンプルコード

```rust
// crates/renderer/src/context.rs
use wgpu::{Device, Queue, Surface, SurfaceConfiguration};
use winit::window::Window;

pub struct RenderContext<'a> {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'a>,
    pub config: SurfaceConfiguration,
}

impl<'a> RenderContext<'a> {
    pub async fn new(window: &'a Window) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapter found"))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("kalidokit-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            }, None)
            .await?;

        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .ok_or_else(|| anyhow::anyhow!("Surface not supported"))?;
        surface.configure(&device, &config);

        Ok(Self { device, queue, surface, config })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}
```

```rust
// crates/renderer/src/skin.rs
use wgpu::{Buffer, Device, Queue};
use glam::Mat4;

/// GPU-side skinning data manager.
/// Bevyのskinning.rsを参考にしたボーン行列管理。
pub struct SkinData {
    joint_buffer: Buffer,
    max_joints: usize,
}

impl SkinData {
    pub fn new(device: &Device, max_joints: usize) -> Self {
        let joint_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("joint_matrices"),
            size: (max_joints * std::mem::size_of::<Mat4>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { joint_buffer, max_joints }
    }

    /// ソルバー結果からボーン行列を計算し、GPUバッファに書き込む。
    /// Bevy の Extract→Prepare ステージに相当。
    pub fn update(&self, queue: &Queue, joint_matrices: &[Mat4]) {
        let data: &[u8] = bytemuck::cast_slice(joint_matrices);
        queue.write_buffer(&self.joint_buffer, 0, data);
    }

    pub fn buffer(&self) -> &Buffer {
        &self.joint_buffer
    }
}
```

```rust
// crates/renderer/src/morph.rs
use wgpu::{Buffer, Device, Queue};

/// GPU-side morph target (BlendShape) weight manager.
pub struct MorphData {
    weight_buffer: Buffer,
    max_targets: usize,
}

impl MorphData {
    pub fn new(device: &Device, max_targets: usize) -> Self {
        let weight_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("morph_weights"),
            size: (max_targets * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { weight_buffer, max_targets }
    }

    /// BlendShape重みをGPUバッファに書き込む。
    /// Blink, A, I, U, E, O の各プリセット値。
    pub fn update(&self, queue: &Queue, weights: &[f32]) {
        let data: &[u8] = bytemuck::cast_slice(weights);
        queue.write_buffer(&self.weight_buffer, 0, data);
    }

    pub fn buffer(&self) -> &Buffer {
        &self.weight_buffer
    }
}
```

```rust
// crates/renderer/src/scene.rs
use crate::context::RenderContext;
use crate::skin::SkinData;
use crate::morph::MorphData;
use crate::mesh::GpuMesh;

pub struct Scene {
    pub meshes: Vec<GpuMesh>,
    pub skin: SkinData,
    pub morph: MorphData,
    pub camera_bind_group: wgpu::BindGroup,
    pub pipeline: wgpu::RenderPipeline,
}

impl Scene {
    /// 1フレーム描画。BevyのRenderステージに相当。
    pub fn render(&self, ctx: &RenderContext) -> anyhow::Result<()> {
        let output = ctx.surface.get_current_texture()?;
        let view = output.texture.create_view(&Default::default());

        let mut encoder = ctx.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("render_encoder") }
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0, g: 1.0, b: 0.0, a: 1.0, // 緑背景 (元実装と同じ)
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None, // TODO: depth buffer
                ..Default::default()
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            // bind_group 1: joints, 2: morph_weights, 3: textures

            for mesh in &self.meshes {
                mesh.draw(&mut render_pass);
            }
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}
```

### 7.2 `vrm` クレート

#### 入出力

| モジュール | 入力 | 出力 |
|-----------|------|------|
| `loader::load` | VRMファイルパス | `VrmModel` (メッシュ, ボーン, BlendShape) |
| `bone::get_bone` | ボーン名 | `Option<&Bone>` (Transform + 子ボーン) |
| `blendshape::set` | プリセット名, f32値 | 内部MorphTarget重み更新 |
| `spring_bone::update` | delta_time | 物理シミュレーション更新 |

#### サンプルコード

```rust
// crates/vrm/src/loader.rs
use gltf::Gltf;
use crate::model::VrmModel;
use crate::bone::HumanoidBones;
use crate::blendshape::BlendShapeGroup;

/// VRMファイル (glTF 2.0 + VRM拡張) をロードする。
/// glTF crateでベースをパースし、VRM拡張JSONを手動解析。
pub fn load(path: &str) -> anyhow::Result<VrmModel> {
    let gltf = Gltf::open(path)?;
    let blob = gltf.blob.as_deref();

    // glTFメッシュ・スキン・アニメーションをパース
    let meshes = parse_meshes(&gltf, blob)?;
    let skins = parse_skins(&gltf, blob)?;

    // VRM拡張 (extensions.VRM) をJSONからパース
    let vrm_ext = gltf
        .document
        .extensions()
        .and_then(|ext| ext.get("VRM"))
        .ok_or_else(|| anyhow::anyhow!("VRM extension not found"))?;

    let humanoid_bones = HumanoidBones::from_vrm_json(vrm_ext)?;
    let blend_shapes = BlendShapeGroup::from_vrm_json(vrm_ext)?;

    Ok(VrmModel {
        meshes,
        skins,
        humanoid_bones,
        blend_shapes,
    })
}

fn parse_meshes(gltf: &Gltf, blob: Option<&[u8]>) -> anyhow::Result<Vec<MeshData>> {
    todo!("Parse glTF meshes with vertex/index/morph data")
}

fn parse_skins(gltf: &Gltf, blob: Option<&[u8]>) -> anyhow::Result<Vec<SkinData>> {
    todo!("Parse glTF skin joints and inverse bind matrices")
}
```

```rust
// crates/vrm/src/bone.rs
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;

/// VRM Humanoid Bone Names (VRM 0.x spec)
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum HumanoidBoneName {
    Hips, Spine, Chest, Neck, Head,
    LeftUpperArm, LeftLowerArm, LeftHand,
    RightUpperArm, RightLowerArm, RightHand,
    LeftUpperLeg, LeftLowerLeg, LeftFoot,
    RightUpperLeg, RightLowerLeg, RightFoot,
    // 指 (各5本 × Proximal/Intermediate/Distal)
    LeftThumbProximal, LeftThumbIntermediate, LeftThumbDistal,
    LeftIndexProximal, LeftIndexIntermediate, LeftIndexDistal,
    LeftMiddleProximal, LeftMiddleIntermediate, LeftMiddleDistal,
    LeftRingProximal, LeftRingIntermediate, LeftRingDistal,
    LeftLittleProximal, LeftLittleIntermediate, LeftLittleDistal,
    RightThumbProximal, RightThumbIntermediate, RightThumbDistal,
    RightIndexProximal, RightIndexIntermediate, RightIndexDistal,
    RightMiddleProximal, RightMiddleIntermediate, RightMiddleDistal,
    RightRingProximal, RightRingIntermediate, RightRingDistal,
    RightLittleProximal, RightLittleIntermediate, RightLittleDistal,
}

pub struct Bone {
    pub node_index: usize,
    pub local_rotation: Quat,
    pub local_position: Vec3,
    pub inverse_bind_matrix: Mat4,
    pub children: Vec<usize>,
}

pub struct HumanoidBones {
    pub bones: HashMap<HumanoidBoneName, Bone>,
    /// glTFノードインデックス → HumanoidBoneName のマッピング
    pub node_to_bone: HashMap<usize, HumanoidBoneName>,
}

impl HumanoidBones {
    pub fn from_vrm_json(vrm_ext: &serde_json::Value) -> anyhow::Result<Self> {
        todo!("Parse humanBones from VRM extension JSON")
    }

    /// ボーン名でボーンを取得
    pub fn get(&self, name: HumanoidBoneName) -> Option<&Bone> {
        self.bones.get(&name)
    }

    /// 全ボーンのローカル回転からスキニング行列を計算
    /// Bevyのextract_skinned_meshesに相当
    pub fn compute_joint_matrices(&self) -> Vec<Mat4> {
        todo!("Compute final joint matrices via forward kinematics")
    }
}
```

```rust
// crates/vrm/src/blendshape.rs

/// VRM BlendShape Preset Names
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum BlendShapePreset {
    Blink, BlinkL, BlinkR,
    A, I, U, E, O,
    Joy, Angry, Sorrow, Fun,
    Neutral,
}

pub struct BlendShapeBinding {
    pub mesh_index: usize,
    pub morph_target_index: usize,
    pub weight: f32,
}

pub struct BlendShapeGroup {
    pub presets: std::collections::HashMap<BlendShapePreset, Vec<BlendShapeBinding>>,
}

impl BlendShapeGroup {
    pub fn from_vrm_json(vrm_ext: &serde_json::Value) -> anyhow::Result<Self> {
        todo!("Parse blendShapeMaster from VRM extension JSON")
    }

    /// プリセットの値を設定 (0.0 - 1.0)
    pub fn set(&mut self, preset: BlendShapePreset, value: f32) {
        // 各プリセットに紐づくMorphTargetの重みを更新
        if let Some(bindings) = self.presets.get_mut(&preset) {
            for binding in bindings {
                binding.weight = value;
            }
        }
    }

    /// 全MorphTargetの重みを配列として取得 (GPU転送用)
    pub fn get_all_weights(&self, num_targets: usize) -> Vec<f32> {
        let mut weights = vec![0.0; num_targets];
        for bindings in self.presets.values() {
            for b in bindings {
                if b.morph_target_index < num_targets {
                    weights[b.morph_target_index] += b.weight;
                }
            }
        }
        weights
    }
}
```

### 7.3 `app` クレート (wgpu版 メインループ)

```rust
// crates/app/src/main.rs
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Window>,
    render_ctx: Option<renderer::context::RenderContext<'static>>,
    vrm_model: Option<vrm::model::VrmModel>,
    tracker: Option<tracker::holistic::HolisticTracker>,
    webcam: Option<nokhwa::Camera>,
    current_rig: RigState,
}

struct RigState {
    face: Option<solver::RiggedFace>,
    pose: Option<solver::RiggedPose>,
    left_hand: Option<solver::RiggedHand>,
    right_hand: Option<solver::RiggedHand>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // ウィンドウ作成 + wgpu初期化 + VRMロード + MLモデル初期化
        let window = event_loop.create_window(
            Window::default_attributes()
                .with_title("KalidoKit Rust - VRM Motion Capture")
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
        ).unwrap();

        // wgpu初期化 (Bevy の RenderPlugin::build 相当)
        // VRMロード (Bevy の AssetServer::load 相当)
        // ONNXモデル初期化
        // Webカメラ初期化

        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                if let Some(ctx) = &mut self.render_ctx {
                    ctx.resize(size.width, size.height);
                }
            }

            WindowEvent::RedrawRequested => {
                // === メインループ (Bevy の Update Schedule 相当) ===

                // 1. カメラフレーム取得
                // let frame = self.webcam.grab_frame();

                // 2. ML推論
                // let landmarks = self.tracker.detect(&frame);

                // 3. ソルバー
                // self.current_rig.face = Some(solver::face::solve(&landmarks.face, &video));
                // self.current_rig.pose = Some(solver::pose::solve(...));

                // 4. Prepare: ボーン行列・BlendShape重みをGPU転送
                //    (Bevy の Prepare Stage 相当)
                // scene.skin.update(&queue, &joint_matrices);
                // scene.morph.update(&queue, &morph_weights);

                // 5. Render: 描画
                //    (Bevy の Render Stage 相当)
                // scene.render(&ctx);

                // 6. 次フレーム要求
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App {
        window: None,
        render_ctx: None,
        vrm_model: None,
        tracker: None,
        webcam: None,
        current_rig: RigState {
            face: None, pose: None, left_hand: None, right_hand: None,
        },
    };
    event_loop.run_app(&mut app).unwrap();
}
```

---

## 8. Cargo.toml (wgpu版)

```toml
[workspace]
resolver = "2"
members = [
    "crates/app",
    "crates/renderer",
    "crates/vrm",
    "crates/solver",
    "crates/tracker",
]

[workspace.dependencies]
wgpu = "24"
winit = "0.30"
glam = "0.29"
gltf = "1.4"
bytemuck = { version = "1.19", features = ["derive"] }
serde_json = "1.0"
ort = "2.0"
nokhwa = { version = "0.10", features = ["input-native"] }
image = "0.25"
ndarray = "0.16"
anyhow = "1.0"
pollster = "0.4"
env_logger = "0.11"
log = "0.4"
```

---

## 9. Bevy版 vs wgpu版 比較

| 観点 | Bevy版 | wgpu版 |
|------|--------|--------|
| **VRM対応** | bevy_vrmプラグイン (0.1) | gltf crate + VRM拡張パーサー自前 |
| **Skinning** | Bevy組み込み | WGSLシェーダー自前 |
| **MorphTarget** | Bevy組み込み | WGSLシェーダー自前 |
| **SpringBone** | 未対応 | 自前実装可能 |
| **バイナリサイズ** | 大 (ECS/UI/Audio等含む) | **小 (必要なものだけ)** |
| **コンパイル時間** | 長い | **短い** |
| **依存クレート数** | 多い (300+) | **少ない (50程度)** |
| **カスタマイズ性** | Plugin制約 | **完全自由** |
| **学習価値** | エンジン利用者レベル | **GPU/グラフィクス深い理解** |
| **開発工数** | 低 | 高 |
| **メンテ労力** | Bevy破壊的変更に追従 | wgpu破壊的変更に追従 |

### wgpu版を選ぶ理由

1. **軽量**: VRMモーキャプに不要な機能 (UI/Audio/Physics/Network) を含まない
2. **コンパイル高速**: Bevyの巨大依存ツリーを回避
3. **学習目的**: GPU/グラフィクスプログラミングの深い理解
4. **完全制御**: レンダリングパイプラインを細部まで制御可能
5. **SpringBone**: Bevy版では未対応だが、自前実装なら自由に追加可能
6. **MToonシェーダー**: VRM特有のトゥーンシェーダーを自前実装可能
