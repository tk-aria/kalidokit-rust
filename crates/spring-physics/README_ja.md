# spring-physics

VRM スプリングボーン向けの揺れもの物理シミュレーション (Rust 実装)。

[VRM SpringBone](https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_springBone-1.0) および Unreal Engine 向けの [KawaiiPhysics](https://github.com/pafuhana1213/KawaiiPhysics) を参考にしています。

## 概要

`spring-physics` は、スケルタルメッシュ上の二次運動 (髪、布、尻尾、アクセサリなどの揺れもの) をシミュレーションする軽量な物理エンジンです。**Verlet 積分**を使用し、剛性・重力・ドラッグ・風の各パラメータを設定可能で、スフィアコライダーによる貫通防止にも対応しています。

主な特徴:

- **Pure Rust** -- 依存クレートは `glam` のみ
- **フレームレート非依存** -- dt ベースの Verlet 積分 (最大タイムステップ制限付き)
- **VRM 互換**データモデル (チェーン、コライダーグループ、チェーン毎の設定)
- **42 ユニットテスト** -- 正常系・境界値・エラー系をカバー

## インストール

ワークスペースクレートとして:

```toml
[dependencies]
spring-physics = { path = "crates/spring-physics" }
```

公開後:

```sh
cargo add spring-physics
```

## クイックスタート

```rust
use spring_physics::{SpringWorld, BoneResult};
use spring_physics::bone::{SpringBone, BoneChain};
use spring_physics::config::SpringConfig;
use spring_physics::collider::{Collider, ColliderShape};
use glam::{Mat4, Vec3};

// 1. SpringWorld を作成
let mut world = SpringWorld::new();

// 2. ボーンチェーンを定義 (例: 髪の毛の束)
let config = SpringConfig {
    stiffness: 1.0,
    gravity_power: 1.0,
    gravity_dir: Vec3::new(0.0, -1.0, 0.0),
    drag_force: 0.4,
    hit_radius: 0.02,
    wind_scale: 0.0,
};

let bones = vec![
    SpringBone::new(2, Some(1), 0.1, Vec3::NEG_Y, Vec3::new(0.0, -0.1, 0.0)),
    SpringBone::new(3, Some(2), 0.1, Vec3::NEG_Y, Vec3::new(0.0, -0.2, 0.0)),
];
let chain = BoneChain::new(bones, config);
world.add_chain(chain);

// 3. コライダーを追加 (例: 頭のスフィア)
world.add_collider(Collider {
    shape: ColliderShape::Sphere { radius: 0.1 },
    offset: Vec3::ZERO,
    node_index: 0,
    world_position: Vec3::ZERO,
});

// 4. 毎フレーム: デルタタイムとスケルトン行列で更新
let node_matrices = vec![Mat4::IDENTITY; 4];
world.update(1.0 / 60.0, &node_matrices);

// 5. 結果を読み取りスケルトンに適用
for result in world.bone_results() {
    // result.node_index  -- スケルトンノードのインデックス
    // result.world_rotation -- 計算されたワールド空間回転
    println!("ノード {} -> 回転 {:?}", result.node_index, result.world_rotation);
}
```

## アーキテクチャ

```
SpringWorld                        (トップレベルコンテナ)
 |
 |-- chains: Vec<BoneChain>        (接続されたボーンのグループ)
 |    |-- bones: Vec<SpringBone>   (個々のジョイント、テール位置を保持)
 |    |-- config: SpringConfig     (剛性、重力、ドラッグ、風、当たり半径)
 |    +-- collider_indices         (world.colliders への参照)
 |
 +-- colliders: Vec<Collider>      (スケルトンノードに紐付くスフィアコライダー)

毎フレームの更新パイプライン:
  1. スケルトン行列からコライダーのワールド位置を更新
  2. 各チェーンの各ボーンに対して:
     a. Verlet 積分    (integrator.rs)  -- 慣性 + 各種力
     b. コライダー解決 (collider.rs)    -- スフィアからの押し出し
     c. 長さ制約       (constraint.rs)  -- ボーン長を維持
  3. ワールド回転を計算 (solver.rs)     -- レストポーズからの差分
  4. BoneResult のベクタを収集し、呼び出し元で適用
```

## モジュール一覧

| モジュール | 説明 |
|-----------|------|
| `config` | `SpringConfig` -- チェーン毎の物理パラメータ (バリデーション/クランプ付き) |
| `bone` | `SpringBone` (単一ジョイント) と `BoneChain` (共有設定を持つ順序付きチェーン) |
| `collider` | `Collider` と `ColliderShape::Sphere` -- 衝突検出と解決 |
| `integrator` | `verlet_step()` -- 剛性・重力・ドラッグ・風を含む Verlet 積分 |
| `constraint` | `length_constraint()` -- 中心からのボーン長を維持 |
| `solver` | `solve_chain()` と `compute_bone_rotation()` -- チェーン毎の物理ステップ |
| `world` | `SpringWorld` -- トップレベル API: `update()`, `bone_results()`, `reset()` |

## テスト

```sh
cargo test -p spring-physics
```

## ライセンス

親プロジェクト (kalidokit-rust) と同一。
