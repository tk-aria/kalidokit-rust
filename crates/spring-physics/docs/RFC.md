# RFC: spring-physics — Verlet-based Spring Bone Physics Engine

- **Status**: Draft
- **Author**: kalidokit-rust team
- **Date**: 2026-03-22
- **References**: [KawaiiPhysics](https://github.com/pafuhana1213/KawaiiPhysics), VRM secondaryAnimation spec, IDOLM@STER graphics Verlet integration

## 1. Summary

`spring-physics` は VRM/glTF モデルの揺れもの（髪、スカート、アクセサリ等）を Verlet 積分ベースで物理シミュレーションする汎用 Rust クレートです。KawaiiPhysics のアルゴリズムを参考に、軽量かつ安定した疑似物理を提供します。

## 2. Motivation

### 問題

現在の `vrm` クレート内の `spring_bone.rs` には以下の問題があります：

1. **初期位置がダミー値**: `bone_length = 0.1`, `initial_tail = (0, -0.1, 0)` 固定。実際のノード位置を使っていない
2. **物理結果がボーンに反映されない**: `update()` で tail 位置を計算するが、ノード回転への変換がない
3. **コライダーのワールド座標変換がない**: offset が local space のまま使用
4. **vrm クレートに密結合**: 物理エンジンとして再利用できない

### 目標

- VRM 非依存の汎用物理エンジンとして切り出す
- 正しい初期位置計算とワールド座標での物理演算
- 物理結果（回転）をボーン階層に正しく反映
- KawaiiPhysics のアルゴリズムに準拠した安定性

## 3. Design

### 3.1 アーキテクチャ

```
spring-physics (汎用, VRM非依存)
    ↑ 依存
vrm (VRM JSON → SpringWorld 変換アダプタ)
    ↑ 依存
app (毎フレーム update + node_transforms への結果適用)
```

`spring-physics` は `glam` のみに依存し、VRM/wgpu/アプリ固有の型を一切含みません。

### 3.2 コアデータモデル

#### SpringBone

物理シミュレーション対象の個々のボーン。

```rust
pub struct SpringBone {
    pub node_index: usize,         // glTF ノードインデックス
    pub parent_index: Option<usize>, // 親ノードインデックス
    pub bone_length: f32,          // 親→子のボーン長
    pub initial_local_dir: Vec3,   // バインドポーズでの親→子方向 (local)
    pub current_tail: Vec3,        // 現在の末端位置 (world)
    pub prev_tail: Vec3,           // 前フレームの末端位置 (world)
    pub world_rotation: Quat,      // 物理計算結果の回転 (world)
}
```

#### BoneChain

同じパラメータを共有するボーンチェーン（VRM の boneGroup に対応）。

```rust
pub struct BoneChain {
    pub bones: Vec<SpringBone>,
    pub config: SpringConfig,
    pub collider_indices: Vec<usize>,  // 参照するコライダーのインデックス
}
```

#### SpringConfig

物理パラメータ。

```rust
pub struct SpringConfig {
    pub stiffness: f32,       // 復元力の強さ (0.0 ~ 4.0)
    pub gravity_power: f32,   // 重力の強さ
    pub gravity_dir: Vec3,    // 重力方向 (通常 (0, -1, 0))
    pub drag_force: f32,      // 空気抵抗 (0.0 ~ 1.0)
    pub hit_radius: f32,      // コライダー衝突半径
    pub wind_scale: f32,      // 風力スケール
}
```

#### Collider

衝突判定用のプリミティブ。

```rust
pub enum ColliderShape {
    Sphere { radius: f32 },
    Capsule { radius: f32, tail: Vec3 },
    Plane { normal: Vec3 },
}

pub struct Collider {
    pub shape: ColliderShape,
    pub offset: Vec3,           // ローカルオフセット
    pub node_index: usize,      // アタッチ先ノード
    pub world_position: Vec3,   // 毎フレーム更新されるワールド位置
}
```

#### SpringWorld

全体を管理するメインコンテナ。

```rust
pub struct SpringWorld {
    pub chains: Vec<BoneChain>,
    pub colliders: Vec<Collider>,
    pub global_gravity: Vec3,
    pub wind: Vec3,
    pub time_scale: f32,
    pub enabled: bool,
}
```

### 3.3 物理ステップ (1フレーム)

```
1. コライダーのワールド座標を更新
   collider.world_position = node_transforms[collider.node_index] * collider.offset

2. 各 BoneChain の各 SpringBone について:
   a. center = 親ノードのワールド位置
   b. velocity = (current_tail - prev_tail) * (1 - drag)
   c. stiffness_force = (initial_world_tail - current_tail).normalize() * stiffness * dt
   d. gravity = gravity_dir * gravity_power * dt
   e. wind = world_wind * wind_scale * dt
   f. next_tail = current_tail + velocity + stiffness_force + gravity + wind
   g. コライダー衝突判定 → 押し出し
   h. ボーン長制約: next_tail = center + (next_tail - center).normalize() * bone_length
   i. prev_tail = current_tail; current_tail = next_tail

3. 各 SpringBone の world_rotation を計算:
   親→子の初期方向と現在方向の差分から回転 Quat を算出
```

### 3.4 KawaiiPhysics との対応

| KawaiiPhysics | spring-physics |
|---------------|----------------|
| Verlet integration | `integrator::verlet_step()` |
| Stiffness (spring) | `SpringConfig::stiffness` |
| Damping | `SpringConfig::drag_force` |
| Gravity | `SpringConfig::gravity_power` + `gravity_dir` |
| Wind | `SpringWorld::wind` + `SpringConfig::wind_scale` |
| Sphere/Capsule/Plane colliders | `ColliderShape` enum |
| Bone length constraint | `constraint::length_constraint()` |

## 4. Module Structure

```
src/
├── lib.rs           SpringWorld (メイン API, pub use)
├── bone.rs          SpringBone, BoneChain
├── collider.rs      ColliderShape, Collider, 衝突判定
├── config.rs        SpringConfig, GlobalConfig
├── constraint.rs    length_constraint()
├── integrator.rs    verlet_step() — Verlet 積分 + 外力計算
└── solver.rs        solve_frame() — 1フレームの物理ステップ全体
```

## 5. Public API

```rust
// 構築
let mut world = SpringWorld::new();
world.add_chain(chain);
world.add_collider(collider);

// VRM からの構築 (vrm クレート側)
let world = SpringWorld::from_bone_groups(groups, node_world_positions);

// 毎フレーム更新
world.update(delta_time, &node_world_matrices);

// 結果取得
for result in world.bone_results() {
    // result.node_index, result.world_rotation
}
```

## 6. Integration Points

### 6.1 vrm クレート

`spring_bone.rs` を `spring-physics` への変換アダプタに変更：

```rust
// vrm::spring_bone
pub fn build_spring_world(
    vrm_json: &Value,
    node_transforms: &[NodeTransform],
) -> Result<SpringWorld, VrmError>
```

### 6.2 app クレート (update.rs)

```rust
// 3.5. Update spring bone physics
if state.spring_physics_enabled {
    state.spring_world.update(delta_time, &node_world_matrices);
    for result in state.spring_world.bone_results() {
        state.vrm_model.node_transforms[result.node_index].rotation = result.world_rotation;
    }
}
```

### 6.3 Lua Settings

```lua
local sp = imgui.checkbox("Spring Physics", avatar.get_spring_physics())
avatar.set_spring_physics(sp)
```

## 7. Risks & Mitigations

| リスク | 対策 |
|--------|------|
| 物理の不安定化 (爆発) | ボーン長制約 + delta_time クランプ (max 0.05s) |
| パフォーマンス (47ボーン × 毎フレーム) | Verlet は O(n) で軽量。必要なら chain 単位の LOD |
| スケルトン伸縮 | KawaiiPhysics 方式: 位置ベースの制約で長さ維持 |
| コライダー貫通 | 反復衝突判定 (2回) + 押し出し方向の正規化 |

## 8. Alternatives Considered

| 選択肢 | 却下理由 |
|--------|---------|
| vrm クレート内で修正 | VRM 非依存にしたい、再利用性が低い |
| rapier3d (物理エンジン) | オーバーキル。揺れものに rigid body は不要 |
| bevy_rapier | bevy 依存。我々のアーキテクチャに合わない |

## 9. Open Questions

1. VRM 1.0 (VRMC_springBone) との互換性はスコープ外とするか？
2. GPU コンピュートシェーダーでの並列化は将来検討か？
3. デバッグ overlay (spring bone の tail 位置可視化) の優先度は？
