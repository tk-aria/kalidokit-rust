# KalidoKit Rust - VRM Avatar Motion Capture Application 設計書

## 1. 概要

Webカメラからの映像をリアルタイムに解析し、顔・体・手のモーションキャプチャを行い、
VRM形式の3Dアバターに反映するデスクトップアプリケーション。

**元実装 (JavaScript):**
- Three.js + @pixiv/three-vrm → VRM描画
- MediaPipe Holistic → 顔468点 + 体33点 + 手21点×2 ランドマーク検出
- KalidoKit → ランドマーク→ボーン回転/ブレンドシェイプ変換ソルバー

**Rust実装:**
- Bevy Engine + bevy_vrm → VRM描画・アニメーション
- ort (ONNX Runtime) → MediaPipeモデル推論
- nokhwa → Webカメラキャプチャ
- glam → 数学演算 (Quaternion/Vector)

---

## 2. E-R図 (Entity-Relationship Diagram)

```
┌─────────────────┐       ┌──────────────────────┐
│   VrmModel       │       │   Camera             │
├─────────────────┤       ├──────────────────────┤
│ PK model_id      │       │ PK camera_id          │
│    file_path     │       │    device_name        │
│    vrm_version   │       │    resolution_w       │
│    meta_info     │       │    resolution_h       │
└────────┬────────┘       │    fps                │
         │ 1              └──────────┬───────────┘
         │                           │ 1
         │ has                       │ captures
         │                           │
         ▼ N                         ▼ N
┌─────────────────┐       ┌──────────────────────┐
│   Bone           │       │   Frame              │
├─────────────────┤       ├──────────────────────┤
│ PK bone_id       │       │ PK frame_id           │
│ FK model_id      │       │ FK camera_id          │
│    bone_name     │       │    timestamp          │
│    rotation_x    │       │    image_data (blob)  │
│    rotation_y    │       └──────────┬───────────┘
│    rotation_z    │                  │ 1
│    rotation_w    │                  │
│    position_x    │                  │ produces
│    position_y    │                  │
│    position_z    │                  ▼ 1
└─────────────────┘       ┌──────────────────────┐
                          │   HolisticResult      │
┌─────────────────┐       ├──────────────────────┤
│   BlendShape     │       │ PK result_id          │
├─────────────────┤       │ FK frame_id           │
│ PK shape_id      │       └──┬───┬───┬───────────┘
│ FK model_id      │          │   │   │
│    preset_name   │          │1  │1  │1
│    value (f32)   │          │   │   │
└─────────────────┘          ▼N  ▼N  ▼N
                    ┌────────┐ ┌────┐ ┌──────────┐
                    │FaceLM  │ │Pose│ │HandLM    │
                    │        │ │ LM │ │          │
                    ├────────┤ ├────┤ ├──────────┤
                    │PK lm_id│ │PK  │ │PK lm_id  │
                    │FK res  │ │lm  │ │FK res_id  │
                    │idx(468)│ │_id │ │side(L/R) │
                    │x,y,z   │ │FK  │ │idx(0-20) │
                    └────────┘ │res │ │x,y,z     │
                               │idx │ └──────────┘
                               │(33)│
                               │x,y,│
                               │z   │
                               └────┘

┌─────────────────┐       ┌──────────────────────┐
│   RiggedFace     │       │   RiggedPose          │
├─────────────────┤       ├──────────────────────┤
│ head: Euler      │       │ Hips: {rot, pos}      │
│ eye_l: f32       │       │ Spine: Euler          │
│ eye_r: f32       │       │ Chest: Euler          │
│ pupil: Vec2      │       │ *UpperArm: Euler      │
│ mouth: {A,I,U,   │       │ *LowerArm: Euler      │
│         E,O}     │       │ *UpperLeg: Euler      │
│ brow: f32        │       │ *LowerLeg: Euler      │
└─────────────────┘       │ *Hand: Euler          │
                          └──────────────────────┘

┌─────────────────┐
│   RiggedHand     │
├─────────────────┤
│ Wrist: Euler     │
│ *Proximal: Euler │
│ *Intermediate    │
│ *Distal: Euler   │
│ (per finger×5)   │
└─────────────────┘
```

---

## 3. シーケンス図

```
┌──────┐  ┌────────┐  ┌────────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐
│Camera│  │Capture │  │  Tracker   │  │  Solver  │  │Animator  │  │Renderer │
│Device│  │(nokhwa)│  │(ort/ONNX)  │  │(kalidokit│  │(Bevy ECS)│  │(Bevy/   │
│      │  │        │  │            │  │  solver) │  │          │  │ wgpu)   │
└──┬───┘  └───┬────┘  └─────┬──────┘  └────┬─────┘  └────┬─────┘  └────┬────┘
   │          │              │              │             │              │
   │ [1] App起動             │              │             │              │
   │          │──────────────│──────────────│─────────────│──────────────│
   │          │              │              │             │  [2] VRMロード │
   │          │              │              │             │──────────────▶│
   │          │              │              │             │  scene.add() │
   │          │              │              │             │◀─────────────│
   │          │              │              │             │              │
   │          │  [3] MLモデルロード           │             │              │
   │          │              │◀─────────────│             │              │
   │          │              │ face_mesh.onnx             │              │
   │          │              │ pose_landmark.onnx         │              │
   │          │              │ hand_landmark.onnx         │              │
   │          │              │              │             │              │
   ║══════════║══ メインループ ═║══════════════║═════════════║══════════════║
   │          │              │              │             │              │
   │ [4] フレーム取得          │              │             │              │
   │─────────▶│              │              │             │              │
   │  raw RGB │              │              │             │              │
   │          │ [5] 前処理     │              │             │              │
   │          │─────────────▶│              │             │              │
   │          │  resize/norm │              │             │              │
   │          │              │              │             │              │
   │          │              │ [6] 推論      │             │              │
   │          │              │──────┐       │             │              │
   │          │              │      │ face  │             │              │
   │          │              │      │ mesh  │             │              │
   │          │              │◀─────┘       │             │              │
   │          │              │──────┐       │             │              │
   │          │              │      │ pose  │             │              │
   │          │              │◀─────┘       │             │              │
   │          │              │──────┐       │             │              │
   │          │              │      │ hand  │             │              │
   │          │              │      │ ×2    │             │              │
   │          │              │◀─────┘       │             │              │
   │          │              │              │             │              │
   │          │              │ [7] ランドマーク│             │              │
   │          │              │─────────────▶│             │              │
   │          │              │ face:468pts  │             │              │
   │          │              │ pose:33pts   │ [8] ソルバー  │              │
   │          │              │ hand:21×2pts │──────┐      │              │
   │          │              │              │      │solve │              │
   │          │              │              │◀─────┘      │              │
   │          │              │              │             │              │
   │          │              │              │ [9] リグ適用  │              │
   │          │              │              │────────────▶│              │
   │          │              │              │ RiggedFace  │              │
   │          │              │              │ RiggedPose  │              │
   │          │              │              │ RiggedHand  │ [10] 描画     │
   │          │              │              │             │─────────────▶│
   │          │              │              │             │ quaternion   │
   │          │              │              │             │ slerp/lerp   │
   │          │              │              │             │ blendshape   │
   │          │              │              │             │◀─────────────│
   ║══════════║══════════════║══════════════║═════════════║══════════════║
   │          │              │              │             │              │
```

### 処理フローの詳細

1. **App起動** - Bevy Appを初期化、ウィンドウ・レンダラー作成
2. **VRMロード** - `bevy_vrm`でVRMファイルをロード、シーンに追加
3. **MLモデルロード** - ONNX Runtime セッション初期化 (face/pose/hand)
4. **フレーム取得** - `nokhwa`でWebカメラから640×480のRGBフレームを取得
5. **前処理** - フレームをモデル入力サイズにリサイズ・正規化
6. **推論** - 顔→ポーズ→手 の順にONNXモデルで推論
7. **ランドマーク出力** - 各モデルから正規化座標のランドマークを取得
8. **ソルバー** - ランドマークからオイラー角/位置/ブレンドシェイプ値を計算
9. **リグ適用** - ボーンのQuaternionをslerp補間で適用、BlendShape値をlerp補間
10. **描画** - Bevyレンダリングパイプラインで画面に描画

---

## 4. ディレクトリ構成図

```
kalidokit-rust/
├── Cargo.toml                     # ワークスペース定義
├── Cargo.lock
├── README.md
├── assets/
│   ├── models/
│   │   └── default_avatar.vrm     # デフォルトVRMアバター
│   └── ml/
│       ├── face_landmark.onnx     # 顔ランドマーク検出モデル
│       ├── pose_landmark.onnx     # ポーズ推定モデル
│       └── hand_landmark.onnx     # 手ランドマーク検出モデル
│
├── crates/
│   ├── app/                       # メインアプリケーション (bin)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs            # Bevyアプリエントリポイント
│   │       ├── plugins/
│   │       │   ├── mod.rs
│   │       │   ├── camera.rs      # カメラキャプチャプラグイン
│   │       │   ├── tracker.rs     # モーショントラッキングプラグイン
│   │       │   └── avatar.rs      # VRMアバター制御プラグイン
│   │       ├── components/
│   │       │   ├── mod.rs
│   │       │   ├── landmarks.rs   # ランドマークコンポーネント
│   │       │   └── rig.rs         # リグ結果コンポーネント
│   │       └── systems/
│   │           ├── mod.rs
│   │           ├── capture.rs     # フレーム取得システム
│   │           ├── inference.rs   # ML推論システム
│   │           ├── solve.rs       # ソルバーシステム
│   │           └── animate.rs     # アニメーション適用システム
│   │
│   ├── solver/                    # ソルバーライブラリ (lib)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs             # ライブラリルート
│   │       ├── face.rs            # 顔ソルバー (head/eye/mouth/pupil)
│   │       ├── pose.rs            # ポーズソルバー (体幹/四肢)
│   │       ├── hand.rs            # 手ソルバー (指関節)
│   │       ├── types.rs           # 共通型定義
│   │       └── utils.rs           # remap/clamp/lerp等ユーティリティ
│   │
│   └── tracker/                   # トラッキングライブラリ (lib)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs             # ライブラリルート
│           ├── face_mesh.rs       # 顔メッシュ検出
│           ├── pose.rs            # ポーズ推定
│           ├── hand.rs            # 手ランドマーク検出
│           ├── holistic.rs        # 統合推論パイプライン
│           └── preprocess.rs      # 画像前処理
│
├── tests/
│   ├── solver_test.rs             # ソルバーユニットテスト
│   └── tracker_test.rs            # トラッカーユニットテスト
│
└── examples/
    ├── simple_tracking.rs         # 最小構成サンプル
    └── custom_avatar.rs           # カスタムVRM読み込みサンプル
```

---

## 5. ライブラリモジュール 入出力 & サンプルコード

### 5.1 `solver` クレート - 顔・体・手のリグソルバー

KalidoKitのRust移植。ランドマーク座標からボーン回転値・BlendShape値を算出する。

#### 入出力

| モジュール | 入力 | 出力 |
|-----------|------|------|
| `face::solve` | `&[Vec3; 468]` (顔ランドマーク), `VideoInfo` | `RiggedFace { head, eye, mouth, pupil }` |
| `pose::solve` | `&[Vec3; 33]` (3Dポーズ), `&[Vec2; 33]` (2Dポーズ), `VideoInfo` | `RiggedPose { Hips, Spine, Arms, Legs, ... }` |
| `hand::solve` | `&[Vec3; 21]` (手ランドマーク), `Side` | `RiggedHand { Wrist, fingers... }` |

#### サンプルコード

```rust
// crates/solver/src/lib.rs
pub mod face;
pub mod hand;
pub mod pose;
pub mod types;
pub mod utils;

pub use types::*;
```

```rust
// crates/solver/src/types.rs
use glam::{EulerRot, Quat, Vec2, Vec3};

/// 動画メタ情報
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
}

/// 左右判定
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// 顔ソルバー結果
#[derive(Debug, Clone)]
pub struct RiggedFace {
    pub head: EulerAngles,
    pub eye: EyeValues,
    pub pupil: Vec2,
    pub mouth: MouthShape,
    pub brow: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct EulerAngles {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl EulerAngles {
    pub fn to_quat(&self) -> Quat {
        Quat::from_euler(EulerRot::XYZ, self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EyeValues {
    pub l: f32,
    pub r: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct MouthShape {
    pub a: f32,
    pub i: f32,
    pub u: f32,
    pub e: f32,
    pub o: f32,
}

/// ポーズソルバー結果
#[derive(Debug, Clone)]
pub struct RiggedPose {
    pub hips: HipTransform,
    pub spine: EulerAngles,
    pub chest: EulerAngles,
    pub right_upper_arm: EulerAngles,
    pub right_lower_arm: EulerAngles,
    pub left_upper_arm: EulerAngles,
    pub left_lower_arm: EulerAngles,
    pub right_upper_leg: EulerAngles,
    pub right_lower_leg: EulerAngles,
    pub left_upper_leg: EulerAngles,
    pub left_lower_leg: EulerAngles,
    pub left_hand: EulerAngles,
    pub right_hand: EulerAngles,
}

#[derive(Debug, Clone)]
pub struct HipTransform {
    pub rotation: EulerAngles,
    pub position: Vec3,
}

/// 手ソルバー結果
#[derive(Debug, Clone)]
pub struct RiggedHand {
    pub wrist: EulerAngles,
    pub thumb_proximal: EulerAngles,
    pub thumb_intermediate: EulerAngles,
    pub thumb_distal: EulerAngles,
    pub index_proximal: EulerAngles,
    pub index_intermediate: EulerAngles,
    pub index_distal: EulerAngles,
    pub middle_proximal: EulerAngles,
    pub middle_intermediate: EulerAngles,
    pub middle_distal: EulerAngles,
    pub ring_proximal: EulerAngles,
    pub ring_intermediate: EulerAngles,
    pub ring_distal: EulerAngles,
    pub little_proximal: EulerAngles,
    pub little_intermediate: EulerAngles,
    pub little_distal: EulerAngles,
}
```

```rust
// crates/solver/src/face.rs
use crate::types::*;
use crate::utils::{clamp, remap};
use glam::Vec3;

/// 顔ランドマーク468点から頭部回転・目・口・瞳孔の値を算出
pub fn solve(landmarks: &[Vec3], video: &VideoInfo) -> RiggedFace {
    let head = calc_head_rotation(landmarks);
    let eye = calc_eye_openness(landmarks);
    let mouth = calc_mouth_shape(landmarks);
    let pupil = calc_pupil_position(landmarks);
    let brow = calc_brow_raise(landmarks);

    RiggedFace { head, eye, pupil, mouth, brow }
}

/// 瞬きを安定化 (頭部傾き補正)
pub fn stabilize_blink(eye: &EyeValues, head_y: f32) -> EyeValues {
    // 頭部のY軸回転に基づいて左右の目の開閉度を調整
    let max_ratio = 0.285;
    let ratio = clamp(head_y / max_ratio, 0.0, 1.0);
    EyeValues {
        l: eye.l + ratio * (eye.r - eye.l),
        r: eye.r + ratio * (eye.l - eye.r),
    }
}

fn calc_head_rotation(lm: &[Vec3]) -> EulerAngles {
    // 鼻先・顎・左右耳の座標から頭部回転を推定
    // (実装省略 - KalidoKitの計算ロジック移植)
    todo!()
}

fn calc_eye_openness(lm: &[Vec3]) -> EyeValues {
    // 上下まぶたのランドマーク間距離から開閉度を計算
    todo!()
}

fn calc_mouth_shape(lm: &[Vec3]) -> MouthShape {
    // 口のランドマークから母音形状 (A/I/U/E/O) を推定
    todo!()
}

fn calc_pupil_position(lm: &[Vec3]) -> glam::Vec2 {
    // 虹彩ランドマーク(468-477)から瞳孔位置を計算
    todo!()
}

fn calc_brow_raise(lm: &[Vec3]) -> f32 {
    // 眉のランドマーク高さから眉上げ度を計算
    todo!()
}
```

```rust
// crates/solver/src/pose.rs
use crate::types::*;
use glam::{Vec2, Vec3};

/// 33点のポーズランドマーク(3D+2D)から各ボーン回転を算出
pub fn solve(
    landmarks_3d: &[Vec3],
    landmarks_2d: &[Vec2],
    video: &VideoInfo,
) -> RiggedPose {
    let hips = calc_hip_transform(landmarks_3d, landmarks_2d, video);
    let spine = calc_spine_rotation(landmarks_3d);

    // 各腕・脚のボーン回転を算出
    let right_upper_arm = calc_limb_rotation(
        landmarks_3d[12], landmarks_3d[14], landmarks_3d[16],
    );
    let right_lower_arm = calc_limb_rotation(
        landmarks_3d[14], landmarks_3d[16], landmarks_3d[18],
    );
    let left_upper_arm = calc_limb_rotation(
        landmarks_3d[11], landmarks_3d[13], landmarks_3d[15],
    );
    let left_lower_arm = calc_limb_rotation(
        landmarks_3d[13], landmarks_3d[15], landmarks_3d[17],
    );

    RiggedPose {
        hips,
        spine: spine.clone(),
        chest: spine,
        right_upper_arm,
        right_lower_arm,
        left_upper_arm,
        left_lower_arm,
        right_upper_leg: calc_limb_rotation(
            landmarks_3d[24], landmarks_3d[26], landmarks_3d[28],
        ),
        right_lower_leg: calc_limb_rotation(
            landmarks_3d[26], landmarks_3d[28], landmarks_3d[30],
        ),
        left_upper_leg: calc_limb_rotation(
            landmarks_3d[23], landmarks_3d[25], landmarks_3d[27],
        ),
        left_lower_leg: calc_limb_rotation(
            landmarks_3d[25], landmarks_3d[27], landmarks_3d[29],
        ),
        left_hand: EulerAngles { x: 0.0, y: 0.0, z: 0.0 },
        right_hand: EulerAngles { x: 0.0, y: 0.0, z: 0.0 },
    }
}

fn calc_hip_transform(lm3d: &[Vec3], lm2d: &[Vec2], video: &VideoInfo) -> HipTransform {
    todo!()
}

fn calc_spine_rotation(lm3d: &[Vec3]) -> EulerAngles {
    todo!()
}

fn calc_limb_rotation(a: Vec3, b: Vec3, c: Vec3) -> EulerAngles {
    // 3つの関節座標から2関節間のオイラー角を算出
    todo!()
}
```

```rust
// crates/solver/src/hand.rs
use crate::types::*;
use glam::Vec3;

/// 21点の手ランドマークから各指関節の回転を算出
pub fn solve(landmarks: &[Vec3], side: Side) -> RiggedHand {
    let wrist = calc_wrist_rotation(landmarks);

    // 各指の3関節 (Proximal/Intermediate/Distal) の回転を算出
    let thumb = calc_finger_rotations(landmarks, &[1, 2, 3, 4]);
    let index = calc_finger_rotations(landmarks, &[5, 6, 7, 8]);
    let middle = calc_finger_rotations(landmarks, &[9, 10, 11, 12]);
    let ring = calc_finger_rotations(landmarks, &[13, 14, 15, 16]);
    let little = calc_finger_rotations(landmarks, &[17, 18, 19, 20]);

    RiggedHand {
        wrist,
        thumb_proximal: thumb[0],
        thumb_intermediate: thumb[1],
        thumb_distal: thumb[2],
        index_proximal: index[0],
        index_intermediate: index[1],
        index_distal: index[2],
        middle_proximal: middle[0],
        middle_intermediate: middle[1],
        middle_distal: middle[2],
        ring_proximal: ring[0],
        ring_intermediate: ring[1],
        ring_distal: ring[2],
        little_proximal: little[0],
        little_intermediate: little[1],
        little_distal: little[2],
    }
}

fn calc_wrist_rotation(lm: &[Vec3]) -> EulerAngles {
    todo!()
}

fn calc_finger_rotations(lm: &[Vec3], indices: &[usize]) -> [EulerAngles; 3] {
    // 4つの関節点から3つの関節回転を計算
    todo!()
}
```

```rust
// crates/solver/src/utils.rs

/// 値を範囲内にクランプ
pub fn clamp(val: f32, min: f32, max: f32) -> f32 {
    val.max(min).min(max)
}

/// 値を元の範囲から新しい範囲にリマップ
pub fn remap(val: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = (val - in_min) / (in_max - in_min);
    out_min + t * (out_max - out_min)
}

/// 線形補間
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Vec3の線形補間
pub fn lerp_vec3(a: glam::Vec3, b: glam::Vec3, t: f32) -> glam::Vec3 {
    a + (b - a) * t
}
```

---

### 5.2 `tracker` クレート - MLモデルによるランドマーク検出

#### 入出力

| モジュール | 入力 | 出力 |
|-----------|------|------|
| `holistic::detect` | `&DynamicImage` (カメラフレーム) | `HolisticResult { face, pose_3d, pose_2d, left_hand, right_hand }` |
| `face_mesh::detect` | `&DynamicImage` | `Option<[Vec3; 468]>` |
| `pose::detect` | `&DynamicImage` | `Option<(Vec<Vec3>, Vec<Vec2>)>` (3D + 2D) |
| `hand::detect` | `&DynamicImage` | `Option<[Vec3; 21]>` |

#### サンプルコード

```rust
// crates/tracker/src/lib.rs
pub mod face_mesh;
pub mod hand;
pub mod holistic;
pub mod pose;
pub mod preprocess;

use glam::{Vec2, Vec3};

/// 統合トラッキング結果
#[derive(Debug, Clone)]
pub struct HolisticResult {
    pub face_landmarks: Option<Vec<Vec3>>,
    pub pose_landmarks_3d: Option<Vec<Vec3>>,
    pub pose_landmarks_2d: Option<Vec<Vec2>>,
    pub left_hand_landmarks: Option<Vec<Vec3>>,
    pub right_hand_landmarks: Option<Vec<Vec3>>,
}
```

```rust
// crates/tracker/src/holistic.rs
use crate::{face_mesh, hand, pose, HolisticResult};
use image::DynamicImage;
use ort::session::Session;

/// 統合推論パイプライン
pub struct HolisticTracker {
    face_session: Session,
    pose_session: Session,
    hand_session: Session,
}

impl HolisticTracker {
    /// ONNXモデルファイルからトラッカーを初期化
    pub fn new(
        face_model_path: &str,
        pose_model_path: &str,
        hand_model_path: &str,
    ) -> anyhow::Result<Self> {
        let face_session = Session::builder()?
            .with_model_from_file(face_model_path)?;
        let pose_session = Session::builder()?
            .with_model_from_file(pose_model_path)?;
        let hand_session = Session::builder()?
            .with_model_from_file(hand_model_path)?;

        Ok(Self {
            face_session,
            pose_session,
            hand_session,
        })
    }

    /// フレームから全ランドマークを検出
    pub fn detect(&self, frame: &DynamicImage) -> anyhow::Result<HolisticResult> {
        let face_landmarks = face_mesh::detect(&self.face_session, frame)?;
        let (pose_3d, pose_2d) = pose::detect(&self.pose_session, frame)?;
        let left_hand = hand::detect(&self.hand_session, frame, true)?;
        let right_hand = hand::detect(&self.hand_session, frame, false)?;

        Ok(HolisticResult {
            face_landmarks,
            pose_landmarks_3d: pose_3d,
            pose_landmarks_2d: pose_2d,
            left_hand_landmarks: left_hand,
            right_hand_landmarks: right_hand,
        })
    }
}
```

```rust
// crates/tracker/src/preprocess.rs
use image::{DynamicImage, RgbImage};
use ndarray::Array4;

/// 画像をモデル入力テンソルに変換
pub fn preprocess_image(
    image: &DynamicImage,
    target_width: u32,
    target_height: u32,
) -> Array4<f32> {
    let resized = image.resize_exact(
        target_width,
        target_height,
        image::imageops::FilterType::Bilinear,
    );
    let rgb = resized.to_rgb8();

    // [1, 3, H, W] テンソルに変換、0-1に正規化
    let mut tensor = Array4::<f32>::zeros((1, 3, target_height as usize, target_width as usize));
    for y in 0..target_height {
        for x in 0..target_width {
            let pixel = rgb.get_pixel(x, y);
            tensor[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
            tensor[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
            tensor[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
        }
    }
    tensor
}
```

---

### 5.3 `app` クレート - Bevyアプリケーション

#### サンプルコード

```rust
// crates/app/src/main.rs
use bevy::prelude::*;
use bevy_vrm::VrmPlugin;

mod components;
mod plugins;
mod systems;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "KalidoKit Rust - VRM Motion Capture".to_string(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VrmPlugin)
        .add_plugins(plugins::camera::CameraCapturePlugin)
        .add_plugins(plugins::tracker::TrackerPlugin)
        .add_plugins(plugins::avatar::AvatarPlugin)
        .run();
}
```

```rust
// crates/app/src/plugins/avatar.rs
use bevy::prelude::*;
use bevy_vrm::VrmBundle;
use crate::components::rig::RigTarget;

pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_avatar)
           .add_systems(Update, apply_rig_to_vrm);
    }
}

fn setup_avatar(mut commands: Commands, asset_server: Res<AssetServer>) {
    // VRMアバターをロード
    commands.spawn((
        VrmBundle {
            vrm: asset_server.load("models/default_avatar.vrm"),
            ..default()
        },
        RigTarget,
    ));

    // カメラ
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.4, 0.7).looking_at(Vec3::new(0.0, 1.4, 0.0), Vec3::Y),
    ));

    // ライト
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            ..default()
        },
        Transform::from_xyz(1.0, 1.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn apply_rig_to_vrm(
    rig_data: Res<crate::components::rig::CurrentRig>,
    mut transforms: Query<&mut Transform, With<RigTarget>>,
) {
    // ソルバー結果をVRMボーンに適用
    // slerp/lerp補間でスムーズなアニメーション
    if let Some(ref face) = rig_data.face {
        // 頭部回転・ブレンドシェイプ適用
    }
    if let Some(ref pose) = rig_data.pose {
        // 体幹・四肢のボーン回転適用
    }
}
```

```rust
// crates/app/src/systems/capture.rs
use bevy::prelude::*;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use image::DynamicImage;

#[derive(Resource)]
pub struct WebcamCapture {
    camera: Camera,
}

impl WebcamCapture {
    pub fn new() -> anyhow::Result<Self> {
        let index = CameraIndex::Index(0);
        let requested = RequestedFormat::new::<RgbFormat>(
            RequestedFormatType::AbsoluteHighestResolution,
        );
        let camera = Camera::new(index, requested)?;
        Ok(Self { camera })
    }

    pub fn grab_frame(&mut self) -> anyhow::Result<DynamicImage> {
        let frame = self.camera.frame()?;
        let image = frame.decode_image::<RgbFormat>()?;
        Ok(DynamicImage::ImageRgb8(image))
    }
}
```

```rust
// crates/app/src/systems/solve.rs
use bevy::prelude::*;
use solver::{face, hand, pose, RiggedFace, RiggedHand, RiggedPose, Side, VideoInfo};
use crate::components::landmarks::CurrentLandmarks;
use crate::components::rig::CurrentRig;

/// ランドマークからリグ値を計算するシステム
pub fn solve_system(
    landmarks: Res<CurrentLandmarks>,
    mut rig: ResMut<CurrentRig>,
) {
    let video = VideoInfo { width: 640, height: 480 };

    // 顔ソルバー
    if let Some(ref face_lm) = landmarks.face {
        let rigged = face::solve(face_lm, &video);
        rig.face = Some(rigged);
    }

    // ポーズソルバー
    if let Some((ref lm3d, ref lm2d)) = landmarks.pose {
        let rigged = pose::solve(lm3d, lm2d, &video);
        rig.pose = Some(rigged);
    }

    // 手ソルバー
    if let Some(ref left) = landmarks.left_hand {
        rig.left_hand = Some(hand::solve(left, Side::Left));
    }
    if let Some(ref right) = landmarks.right_hand {
        rig.right_hand = Some(hand::solve(right, Side::Right));
    }
}
```

---

### 5.4 ワークスペース Cargo.toml

```toml
# Cargo.toml (ワークスペースルート)
[workspace]
resolver = "2"
members = [
    "crates/app",
    "crates/solver",
    "crates/tracker",
]

[workspace.dependencies]
bevy = "0.16"
bevy_vrm = "0.1"
glam = "0.29"
ort = "2.0"
nokhwa = { version = "0.10", features = ["input-native"] }
image = "0.25"
ndarray = "0.16"
anyhow = "1.0"
```

```toml
# crates/solver/Cargo.toml
[package]
name = "solver"
version = "0.1.0"
edition = "2021"

[dependencies]
glam = { workspace = true }
```

```toml
# crates/tracker/Cargo.toml
[package]
name = "tracker"
version = "0.1.0"
edition = "2021"

[dependencies]
ort = { workspace = true }
image = { workspace = true }
ndarray = { workspace = true }
glam = { workspace = true }
anyhow = { workspace = true }
```

```toml
# crates/app/Cargo.toml
[package]
name = "kalidokit-rust"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { workspace = true }
bevy_vrm = { workspace = true }
nokhwa = { workspace = true }
image = { workspace = true }
glam = { workspace = true }
anyhow = { workspace = true }
solver = { path = "../solver" }
tracker = { path = "../tracker" }
```

---

## 6. 依存クレート一覧

| クレート | バージョン | 用途 | 状態 |
|---------|-----------|------|------|
| [bevy](https://bevy.org/) | 0.16 | ECSゲームエンジン/レンダリング | Production Ready |
| [bevy_vrm](https://lib.rs/crates/bevy_vrm) | 0.1 | VRM 0.0/1.0 ローダー | Active Dev |
| [ort](https://github.com/pykeio/ort) | 2.0 | ONNX Runtime 推論 | Production Ready |
| [nokhwa](https://github.com/l1npengtul/nokhwa) | 0.10 | Webカメラキャプチャ | Active Dev |
| [glam](https://crates.io/crates/glam) | 0.29 | 数学演算 (Vec/Quat/Euler) | Production Ready |
| [image](https://crates.io/crates/image) | 0.25 | 画像処理 | Production Ready |
| [ndarray](https://crates.io/crates/ndarray) | 0.16 | テンソル操作 | Production Ready |
| [gltf](https://github.com/gltf-rs/gltf) | 1.4 | glTF 2.0パーサー(bevy_vrm依存) | Production Ready |

---

## 7. 元実装 (JS) → Rust 対応表

| JS (元実装) | Rust (新実装) | 備考 |
|------------|--------------|------|
| Three.js | Bevy Engine (wgpu) | ECSベースで構造化 |
| @pixiv/three-vrm | bevy_vrm | VRM 0.0/1.0対応 |
| MediaPipe Holistic | ort + ONNXモデル | ブラウザAPI→ネイティブ推論 |
| KalidoKit | solver クレート | Rust移植、同一アルゴリズム |
| Camera Utils | nokhwa | ネイティブカメラアクセス |
| requestAnimationFrame | Bevy Update system | ECSシステムスケジュール |
| THREE.Quaternion.slerp | glam::Quat::slerp | 同一数学操作 |
| BlendShapeProxy | bevy_vrm BlendShape | VRM仕様準拠 |

---

## 8. 実装上の重要な注意点

元実装のソースコード分析から判明した、Rust移植時に注意すべきポイント:

### 8.1 補間パラメータ (Dampener / Lerp Amount)

各ボーンには異なる補間パラメータが設定されている。これを正確に再現しないとアバターの動きが不自然になる。

| ボーン | Dampener | Lerp Amount | 備考 |
|--------|----------|-------------|------|
| Neck (頭部) | 0.7 | 0.3 | 70%減衰 |
| Hips (回転) | 0.7 | 0.3 | 70%減衰 |
| Hips (位置) | 1.0 | **0.07** | 非常に遅い補間 |
| Chest | **0.25** | 0.3 | 大幅に減衰 |
| Spine | **0.45** | 0.3 | 中程度の減衰 |
| 四肢 (上腕/前腕/大腿/下腿) | 1.0 | 0.3 | フル回転 |
| 指関節 | 1.0 | 0.3 | フル回転 |
| 目の瞬き | - | 0.5 | BlendShape補間 |
| 口の母音 | - | 0.5 | BlendShape補間 |
| 瞳孔 | - | 0.4 | Euler補間 |

### 8.2 座標系変換の罠

```rust
// Hip位置はX/Z軸を反転し、Y軸に+1.0のオフセットが必要
let hip_position = Vec3::new(
    -rigged_pose.hips.position.x,  // X反転
    rigged_pose.hips.position.y + 1.0,  // Y+1オフセット
    -rigged_pose.hips.position.z,  // Z反転
);
```

### 8.3 目の開閉度の反転

VRMとKalidoKitで目の開閉値の意味が逆:
- **KalidoKit**: 1 = 開いている, 0 = 閉じている
- **VRM BlendShape**: 1 = 閉じている, 0 = 開いている

```rust
// 反転してからクランプ
let eye_l = clamp(1.0 - rigged_face.eye.l, 0.0, 1.0);
```

### 8.4 瞳孔軸のスワップ

瞳孔のX/Y軸がEuler角のY/Xに対応する (逆):
```rust
let look_target = EulerAngles {
    x: lerp(old_target.x, rigged_face.pupil.y, 0.4),  // pupil.y → euler.x
    y: lerp(old_target.y, rigged_face.pupil.x, 0.4),  // pupil.x → euler.y
    z: 0.0,
};
```

### 8.5 手ランドマークの左右反転

カメラがミラー表示のため、MediaPipeの `rightHandLandmarks` が実際の左手に対応:
```rust
// 注意: MediaPipeの出力は鏡像
let left_hand_landmarks = results.right_hand_landmarks;   // 逆
let right_hand_landmarks = results.left_hand_landmarks;   // 逆
```

### 8.6 手首回転の合成

手首はポーズソルバーのZ軸回転 + ハンドソルバーのX/Y軸回転を合成:
```rust
let left_hand_rotation = EulerAngles {
    x: rigged_left_hand.wrist.x,      // ハンドソルバーから
    y: rigged_left_hand.wrist.y,      // ハンドソルバーから
    z: rigged_pose.left_hand.z,       // ポーズソルバーから
};
```

### 8.7 VRMモデルの初期回転

VRMモデルはデフォルトでカメラに背を向けているため、Y軸に180度回転が必要:
```rust
// VRMロード後
vrm_transform.rotation = Quat::from_rotation_y(std::f32::consts::PI);
```

### 8.8 `results.ea` の正体

元実装の `results.ea` は MediaPipe Holistic の **minified** プロパティ名で、
実際は `poseWorldLandmarks` (3Dポーズランドマーク、メートル単位、Hip基準) を指す。
Rust実装ではONNXモデルから直接出力を取得するため、この問題は発生しない。

### 8.9 MediaPipe Holistic 設定

元実装の推論パラメータ:
```rust
pub struct HolisticConfig {
    pub model_complexity: u8,            // 1 (0=lite, 1=full, 2=heavy)
    pub smooth_landmarks: bool,          // true
    pub min_detection_confidence: f32,   // 0.7
    pub min_tracking_confidence: f32,    // 0.7
    pub refine_face_landmarks: bool,     // true (虹彩追跡有効 → 478点)
}
```

### 8.10 リスクと代替案

| リスク | 詳細 | 代替案 |
|--------|------|--------|
| bevy_vrm が0.1.0 | VRMエコシステムが未成熟 | gltf crateで直接VRM拡張をパース |
| MediaPipe ONNX変換 | 公式ONNXモデルが限定的 | RTMPose (usls crate) をポーズ推定に使用 |
| 顔ランドマーク478点 | 虹彩含むモデルの入手性 | SCRFD (rusty_scrfd) + 別途顔メッシュモデル |
| nokhwa安定性 | プラットフォーム差異 | opencv-rust (0.98.x) をフォールバックに |

---

## 9. VRM ボーンマッピング一覧

元実装で使用される全VRMボーン名:

```
Head/Neck系:    Neck
体幹系:        Hips, Spine, Chest
腕系:          LeftUpperArm, LeftLowerArm, RightUpperArm, RightLowerArm
脚系:          LeftUpperLeg, LeftLowerLeg, RightUpperLeg, RightLowerLeg
手首:          LeftHand, RightHand
左指:          LeftThumb{Proximal,Intermediate,Distal}
               LeftIndex{Proximal,Intermediate,Distal}
               LeftMiddle{Proximal,Intermediate,Distal}
               LeftRing{Proximal,Intermediate,Distal}
               LeftLittle{Proximal,Intermediate,Distal}
右指:          RightThumb{Proximal,Intermediate,Distal}
               RightIndex{Proximal,Intermediate,Distal}
               RightMiddle{Proximal,Intermediate,Distal}
               RightRing{Proximal,Intermediate,Distal}
               RightLittle{Proximal,Intermediate,Distal}
```

## 10. VRM BlendShape プリセット一覧

```
Blink    - 両目の瞬き (左目の値を両目に適用)
A        - 母音「あ」
I        - 母音「い」
U        - 母音「う」
E        - 母音「え」
O        - 母音「お」
```
