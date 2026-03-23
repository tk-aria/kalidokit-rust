# Spring Physics 問題診断 & 修正計画

## 現象

Spring Physics ON 時にアバターの髪・スカート等が「暴風が吹き荒れたように」激しく動き、静止しない。

## 原因分析

KawaiiPhysics のソースコード（AnimNode_KawaiiPhysics.cpp）と我々の実装を比較し、以下の3つの根本的な差異を特定した。

---

### 問題1: Stiffness の計算方法が根本的に異なる

**KawaiiPhysics:**
```cpp
// PoseLocation = 現フレームのFK結果（アニメーション後のノード位置）
const FVector BaseLocation = ParentBone.Location + (Bone.PoseLocation - ParentBone.PoseLocation);
Bone.Location += (BaseLocation - Bone.Location) * (1 - pow(1 - Stiffness, Exponent));
```
- `BaseLocation` はアニメーション後の「あるべき位置」
- 現在位置を BaseLocation に向けて「割合ベース」で引っ張る
- `pow(1 - Stiffness, Exponent)` で FPS 非依存にしている

**我々の実装:**
```rust
let stiffness_force = (initial_world_tail - current).normalize_or_zero() * config.stiffness * dt;
```
- **方向のみ**で距離を考慮していない（normalize で距離情報が消える）
- 距離に関わらず一定の力しかかからないため、遠くに行っても復元力が弱く、近くても強すぎる
- FPS 依存（dt に比例）

**影響**: 復元力が距離比例でないため、ボーンが rest position 周辺で安定せず振動し続ける。

---

### 問題2: 回転の適用先が間違っている

**KawaiiPhysics:**
```cpp
// 子→親のベクトルの変位から、**親ボーン**の回転を計算
FVector PoseVector = Bone.PoseLocation - ParentBone.PoseLocation;
FVector SimulateVector = Bone.Location - ParentBone.Location;
FQuat SimulateRotation = FQuat::FindBetweenVectors(PoseVector, SimulateVector) * ParentBone.PoseRotation;

// 親ボーンの回転として設定（子ボーンではない！）
OutBoneTransforms[Bone.ParentIndex].Transform.SetRotation(SimulateRotation);
```
- 子ボーンの位置変化から**親ボーンの回転**を更新する
- 親の回転を変えることで、その先の全ての子ボーンが連動して動く

**我々の実装:**
```rust
// 子ボーン自体の回転を設定
state.vrm_model.node_transforms[result.node_index].rotation = result.local_rotation;
```
- 子ボーン（spring bone ノード自体）の回転を直接設定している
- 子のローカル回転を変えても、子のワールド位置は変わらない（子の位置は親の変換 × 子のローカル translation で決まる）

**影響**: 回転の適用先が間違っているため、見た目にほとんど効果がないか、予期しない変形が起きる。

---

### 問題3: 初期位置の不一致

**KawaiiPhysics:**
- 毎フレーム `PoseLocation` を FK から直接取得（常に最新）
- 初期化時のキャッシュと runtime の不一致問題がない

**我々の実装:**
- `build_spring_world()` で translation-only BFS による近似位置を使用
- runtime の `compute_world_matrices()` は回転を含む正確な FK
- 不一致 = 0.006〜0.01（ボーン長の 7-12%）→ 初期フレームから stiffness 力が発生

**影響**: 起動直後からボーンが「あるべき場所」にいないため、stiffness が常に力を加え続ける。

---

## 修正計画

### 修正1: Stiffness を KawaiiPhysics 方式に変更

**対象ファイル**: `crates/spring-physics/src/integrator.rs`

**変更内容**:
```rust
// Before (方向ベース × dt):
let stiffness_force = (initial_world_tail - current).normalize_or_zero() * config.stiffness * dt;
current + velocity + stiffness_force + gravity + wind_force

// After (位置ベース × 割合):
// Step 1: velocity + external forces
let next = current + velocity + gravity + wind_force;
// Step 2: stiffness (pull toward rest position by fraction)
let exponent = dt * 60.0; // normalize to 60fps
let stiffness_factor = 1.0 - (1.0 - config.stiffness).powf(exponent);
next + (initial_world_tail - next) * stiffness_factor
```

### 修正2: 回転を親ボーンに適用する

**対象ファイル**: `crates/spring-physics/src/solver.rs`, `crates/spring-physics/src/world.rs`, `crates/app/src/update.rs`

**変更内容**:
```rust
// BoneResult を「親ボーンのインデックス + 回転」に変更
pub struct BoneResult {
    pub parent_node_index: usize,  // 回転を適用する先は「親」
    pub rotation: Quat,            // Component Space の回転
}

// compute_bone_rotation:
let pose_vector = bone.pose_location - parent_bone.pose_location; // FK方向
let sim_vector = bone.current_tail - parent_bone.current_location; // 物理方向
let delta_rotation = Quat::from_rotation_arc(
    pose_vector.normalize(),
    sim_vector.normalize(),
);
let rotation = delta_rotation * parent_bone.pose_rotation;
```

**update.rs の適用**:
```rust
for result in spring_world.bone_results() {
    // 親ボーンの回転を更新
    node_transforms[result.parent_node_index].rotation = result.rotation;
}
```

### 修正3: 毎フレーム PoseLocation を FK から取得

**対象ファイル**: `crates/spring-physics/src/bone.rs`, `crates/spring-physics/src/solver.rs`

**変更内容**:
- `SpringBone` に `pose_location: Vec3` フィールドを追加
- 毎フレーム update の最初に、`node_world_matrices` から `pose_location` を更新
- stiffness の `initial_world_tail` を `pose_location` で置き換え
- 初期化時の不一致問題が自動的に解消

```rust
// solver.rs の solve_chain 冒頭:
for bone in &mut chain.bones {
    // 毎フレーム、FKの結果をpose_locationとして保存
    if bone.node_index < node_world_matrices.len() {
        bone.pose_location = node_world_matrices[bone.node_index]
            .transform_point3(Vec3::ZERO);
    }
}
```

---

## 修正順序

1. **修正3** (pose_location): 初期位置不一致を解消。影響が最も小さく安全。
2. **修正1** (stiffness): 物理の安定性が大幅に向上。振動問題の主因を修正。
3. **修正2** (回転適用先): 正しいボーンに回転を適用。視覚的な結果が正しくなる。

各修正後にログで数値を確認し、段階的に検証する。

---

## 検証基準

| # | 基準 | 数値 |
|---|------|------|
| 1 | 起動後5フレームで drift < 0.001 | init_mismatch → 0 |
| 2 | 静止時の回転角度 < 0.5° | angle → ~0° |
| 3 | 頭を動かした後、1秒以内に振動が収束 | drift → 0 within 60 frames |
| 4 | 視覚的に髪が自然に揺れて静止する | 目視確認 |
