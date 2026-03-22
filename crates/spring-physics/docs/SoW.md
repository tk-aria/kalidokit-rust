# Statement of Work: spring-physics クレート実装

- **Project**: kalidokit-rust / spring-physics
- **Date**: 2026-03-22
- **RFC**: [RFC.md](./RFC.md)

## 1. Scope

VRM モデルの揺れもの（髪 14本、猫耳 2本、胸 2、スカート 24、コートスカート 6、袖 4 = 計 47 ボーン）を Verlet ベースの疑似物理で動作させる。

### In Scope

- `spring-physics` クレートの新規作成
- Verlet 積分 + ボーン長制約 + コライダー衝突
- VRM `secondaryAnimation` JSON からの初期化
- 毎フレームの物理更新 + ボーン回転結果の `node_transforms` 反映
- Settings (Lua) からの ON/OFF トグル

### Out of Scope

- VRM 1.0 (`VRMC_springBone`) サポート
- GPU コンピュートシェーダーでの並列化
- Capsule / Plane コライダー（Phase 1 では Sphere のみ）
- 風力シミュレーション（Phase 1 では定数風力のみ）

## 2. Deliverables

| # | 成果物 | 説明 |
|---|--------|------|
| D1 | `crates/spring-physics/` | 新規クレート（Cargo.toml, src/, tests） |
| D2 | `crates/vrm/src/spring_bone.rs` 改修 | `spring-physics` への変換アダプタ |
| D3 | `crates/app/src/update.rs` 改修 | 物理更新 + 結果適用の統合 |
| D4 | Lua Settings トグル | `avatar.get/set_spring_physics()` |
| D5 | features.md 更新 | Phase 20 追加 |

## 3. Work Breakdown Structure

### Phase 1: クレート基盤 (D1)

| Task | ファイル | 内容 |
|------|---------|------|
| 1.1 | `Cargo.toml` | クレート定義 (依存: glam のみ) |
| 1.2 | `src/config.rs` | `SpringConfig` 構造体 |
| 1.3 | `src/bone.rs` | `SpringBone`, `BoneChain` 構造体 |
| 1.4 | `src/collider.rs` | `Collider`, `ColliderShape::Sphere`, 衝突判定 |
| 1.5 | `src/integrator.rs` | `verlet_step()` — Verlet 積分 + 外力 |
| 1.6 | `src/constraint.rs` | `length_constraint()` — ボーン長維持 |
| 1.7 | `src/solver.rs` | `solve_frame()` — 1フレームの全体フロー |
| 1.8 | `src/lib.rs` | `SpringWorld` (メイン API) + re-export |
| 1.9 | テスト | 各モジュールの unit test |

**完了条件**: `cargo test -p spring-physics` 全パス、`cargo check` 通過

### Phase 2: VRM アダプタ (D2)

| Task | ファイル | 内容 |
|------|---------|------|
| 2.1 | `vrm/src/spring_bone.rs` | `build_spring_world()` 関数: VRM JSON + node_transforms → SpringWorld |
| 2.2 | 初期位置計算 | ノードのワールド座標から `bone_length`, `initial_local_dir` を計算 |
| 2.3 | コライダー構築 | colliderGroups → `Collider` with `ColliderShape::Sphere` |
| 2.4 | `vrm/Cargo.toml` | `spring-physics` 依存追加 |

**完了条件**: `default_avatar.vrm` から 22 グループ, 47 ボーン, 12 コライダーグループが正しくパースされる

### Phase 3: 物理更新の統合 (D3)

| Task | ファイル | 内容 |
|------|---------|------|
| 3.1 | `app/src/state.rs` | `AppState` に `spring_world: SpringWorld` 追加 |
| 3.2 | `app/src/init.rs` | VRM ロード後に `build_spring_world()` |
| 3.3 | `app/src/update.rs` | 毎フレーム: `spring_world.update(dt, node_matrices)` |
| 3.4 | 回転結果の反映 | `bone_results()` → `node_transforms[i].rotation` |
| 3.5 | GPU バッファ更新 | スキニング行列の再計算タイミング調整 |

**完了条件**: 頭を動かすとアバターの髪が揺れる

### Phase 4: Settings 統合 (D4, D5)

| Task | ファイル | 内容 |
|------|---------|------|
| 4.1 | `avatar-sdk/src/state.rs` | `spring_physics_enabled: bool` |
| 4.2 | `app/src/lua_avatar.rs` | `avatar.get/set_spring_physics()` |
| 4.3 | `assets/scripts/settings.lua` | トグル追加 |
| 4.4 | `features.md` | Phase 20 タスクリスト |

**完了条件**: Settings から ON/OFF でき、OFF 時は物理計算がスキップされる

## 4. Dependencies

```
spring-physics ← glam (workspace)
vrm ← spring-physics, glam, gltf, serde_json
app ← vrm, spring-physics
```

## 5. Acceptance Criteria

| # | 条件 | 検証方法 |
|---|------|---------|
| AC1 | `cargo test -p spring-physics` 全パス | CI |
| AC2 | `cargo check --workspace` 通過 | CI |
| AC3 | アプリ起動時に spring bone グループがログ出力される | 目視 |
| AC4 | 頭を左右に振ると髪が追従して揺れる | 目視 |
| AC5 | 髪がコライダー（頭・首・腕）を貫通しない | 目視 |
| AC6 | スカートが脚コライダーを貫通しない | 目視 |
| AC7 | Settings から spring physics を OFF にすると揺れが止まる | 目視 |
| AC8 | 60fps でフレーム落ちしない (47ボーン + 12コライダーグループ) | プロファイラ |

## 6. E-R Diagram

```
┌─────────────┐       ┌──────────────┐
│ SpringWorld │──1:N──│  BoneChain   │
│             │       │              │
│ colliders[] │       │ bones[]      │──1:N──┌────────────┐
│ wind        │       │ config       │       │ SpringBone │
│ time_scale  │       │ collider_idx │       │            │
│ enabled     │       └──────────────┘       │ node_index │
└──────┬──────┘                              │ parent_idx │
       │                                     │ bone_length│
       │ 1:N                                 │ current_tail│
       │                                     │ prev_tail  │
┌──────▼──────┐                              │ world_rot  │
│  Collider   │                              └────────────┘
│             │
│ shape       │──enum──┌─ Sphere { radius }
│ offset      │        ├─ Capsule { radius, tail }
│ node_index  │        └─ Plane { normal }
│ world_pos   │
└─────────────┘

┌──────────────┐
│ SpringConfig │ (shared per BoneChain)
│              │
│ stiffness    │
│ gravity_power│
│ gravity_dir  │
│ drag_force   │
│ hit_radius   │
│ wind_scale   │
└──────────────┘
```

## 7. Sequence Diagram (1 Frame)

```
┌─────┐      ┌─────────────┐      ┌──────────┐      ┌──────────┐
│ App │      │ SpringWorld │      │  Solver  │      │ Integrator│
└──┬──┘      └──────┬──────┘      └────┬─────┘      └─────┬────┘
   │                │                   │                   │
   │ update(dt,     │                   │                   │
   │  node_matrices)│                   │                   │
   │───────────────>│                   │                   │
   │                │                   │                   │
   │                │ update_collider   │                   │
   │                │ _world_positions()│                   │
   │                │──────────────────>│                   │
   │                │                   │                   │
   │                │ for each chain:   │                   │
   │                │  solve_chain()    │                   │
   │                │──────────────────>│                   │
   │                │                   │                   │
   │                │                   │ for each bone:    │
   │                │                   │  verlet_step()    │
   │                │                   │──────────────────>│
   │                │                   │                   │
   │                │                   │  check_colliders()│
   │                │                   │<──────────────────│
   │                │                   │                   │
   │                │                   │  length_constraint│
   │                │                   │<──────────────────│
   │                │                   │                   │
   │                │ compute_rotations │                   │
   │                │<──────────────────│                   │
   │                │                   │                   │
   │ bone_results() │                   │                   │
   │<───────────────│                   │                   │
   │                │                   │                   │
   │ apply to       │                   │                   │
   │ node_transforms│                   │                   │
   │                │                   │                   │
```

## 8. Risk Register

| ID | リスク | 影響 | 確率 | 対策 |
|----|--------|------|------|------|
| R1 | 物理不安定化 (爆発) | 高 | 中 | dt クランプ, ボーン長制約, velocity クランプ |
| R2 | ノード変換行列の取得方法 | 中 | 高 | VrmModel.node_transforms を Mat4 で公開する API 追加が必要 |
| R3 | スキニング行列更新タイミング | 中 | 中 | spring bone 更新後 → GPU バッファ更新前の順序を保証 |
| R4 | コライダー貫通 | 低 | 中 | 衝突判定を 2 回反復実行 |
| R5 | パフォーマンス | 低 | 低 | 47 ボーンの Verlet は < 0.1ms |

## 9. Timeline

| Week | Phase | マイルストーン |
|------|-------|--------------|
| 1 | Phase 1 | `spring-physics` クレート完成, テストパス |
| 1 | Phase 2 | VRM アダプタ, 47 ボーン正しくパース |
| 2 | Phase 3 | 統合完了, 髪が揺れる |
| 2 | Phase 4 | Settings トグル, features.md 更新 |
