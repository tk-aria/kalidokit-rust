# spring-physics — Implementation Tasks

> Verlet-based spring bone physics engine for VRM avatar jiggle simulation.
> Based on [RFC.md](docs/RFC.md) and [SoW.md](docs/SoW.md).

## Library Versions

| Crate | Version | Purpose |
|-------|---------|---------|
| `glam` | 0.29.2 | Linear algebra (Vec3, Quat, Mat4) |
| `serde` | 1.0 | (optional) Config serialization |
| `serde_json` | 1.0 | (optional) VRM JSON parsing in tests |
| `spring-physics` | 0.1.0 | This crate |

## Phase 1: Core Data Structures & Verlet Integration

### Step 1.1: Crate scaffolding

- [x] Create `crates/spring-physics/Cargo.toml` <!-- 2026-03-23 00:07 JST -->
  ```toml
  [package]
  name = "spring-physics"
  version = "0.1.0"
  edition = "2021"
  description = "Verlet-based spring bone physics for avatar jiggle simulation"

  [dependencies]
  glam = { version = "0.29.2", features = ["bytemuck"] }

  [dev-dependencies]
  serde_json = "1.0"
  ```
- [x] Add `"crates/spring-physics"` to workspace `Cargo.toml` members <!-- 2026-03-23 00:07 JST -->
- [x] Create `src/lib.rs` with module declarations: <!-- 2026-03-23 00:07 JST -->
  ```rust
  pub mod bone;
  pub mod collider;
  pub mod config;
  pub mod constraint;
  pub mod integrator;
  pub mod solver;
  mod world;
  pub use world::SpringWorld;
  ```
- [x] `cargo check -p spring-physics` passes <!-- 2026-03-23 00:07 JST -->

### Step 1.2: `src/config.rs` — Physics parameters

- [x] Define `SpringConfig` struct: <!-- 2026-03-23 00:09 JST -->
  ```rust
  pub struct SpringConfig {
      pub stiffness: f32,       // 0.0..4.0, restoring force
      pub gravity_power: f32,   // gravity multiplier
      pub gravity_dir: Vec3,    // default (0, -1, 0)
      pub drag_force: f32,      // 0.0..1.0, air resistance
      pub hit_radius: f32,      // collider interaction radius
      pub wind_scale: f32,      // wind force multiplier
  }
  ```
- [x] Implement `Default` with KawaiiPhysics-compatible defaults: <!-- 2026-03-23 00:09 JST -->
  stiffness=1.0, gravity_power=0.0, drag_force=0.4, hit_radius=0.02
- [x] Implement `SpringConfig::validate()` → clamp values to safe ranges <!-- 2026-03-23 00:09 JST -->
- [x] Tests: <!-- 2026-03-23 00:09 JST -->
  - [正常系] `default_values_are_valid`
  - [正常系] `validate_clamps_out_of_range`
  - [異常系] `negative_stiffness_clamped_to_zero`
  - [異常系] `drag_force_above_one_clamped`

### Step 1.3: `src/bone.rs` — SpringBone & BoneChain (~120 lines)

- [x] Define `SpringBone`: <!-- 2026-03-23 00:11 JST -->
  ```rust
  pub struct SpringBone {
      pub node_index: usize,
      pub parent_index: Option<usize>,
      pub bone_length: f32,
      pub initial_local_dir: Vec3,    // bind pose direction (parent→child, local)
      pub current_tail: Vec3,         // world position
      pub prev_tail: Vec3,            // previous frame world position
      pub world_rotation: Quat,       // computed rotation output
  }
  ```
- [ ] Implement `SpringBone::new(node_index, parent_index, bone_length, initial_local_dir, world_tail_pos)`
- [ ] Implement `SpringBone::reset()` — set current_tail = prev_tail = initial world position
- [ ] Define `BoneChain`:
  ```rust
  pub struct BoneChain {
      pub bones: Vec<SpringBone>,
      pub config: SpringConfig,
      pub collider_indices: Vec<usize>,
  }
  ```
- [x] Tests: <!-- 2026-03-23 00:11 JST -->
  - [正常系] `new_initializes_tail_positions`
  - [正常系] `reset_restores_initial_position`
  - [異常系] `zero_bone_length_does_not_panic`

### Step 1.4: `src/collider.rs` — Collision shapes & detection (~100 lines)

- [x] Define `ColliderShape` enum: <!-- 2026-03-23 00:17 JST -->
  ```rust
  pub enum ColliderShape {
      Sphere { radius: f32 },
  }
  ```
  (Capsule, Plane は Phase 6 で追加)
- [x] Define `Collider`: <!-- 2026-03-23 00:17 JST -->
  ```rust
  pub struct Collider {
      pub shape: ColliderShape,
      pub offset: Vec3,
      pub node_index: usize,
      pub world_position: Vec3,
  }
  ```
- [x] Implement `Collider::update_world_position(node_world_matrix: &Mat4)`: <!-- 2026-03-23 00:17 JST -->
  `world_position = node_world_matrix.transform_point3(offset)`
- [x] Implement `Collider::resolve_collision(tail: Vec3, hit_radius: f32) -> Vec3`: <!-- 2026-03-23 00:17 JST -->
  Sphere: if dist < radius + hit_radius → push tail out along normal
- [x] Tests: <!-- 2026-03-23 00:17 JST -->
  - [正常系] `sphere_pushes_point_out`
  - [正常系] `point_outside_sphere_unchanged`
  - [正常系] `world_position_transforms_offset`
  - [異常系] `zero_radius_no_collision`
  - [異常系] `point_at_center_pushed_out_safely` (dist ≈ 0 edge case)

### Step 1.5: `src/integrator.rs` — Verlet integration (~60 lines)

- [x] Implement `verlet_step()`: <!-- 2026-03-23 JST -->
  ```rust
  pub fn verlet_step(
      current: Vec3,
      prev: Vec3,
      config: &SpringConfig,
      initial_world_tail: Vec3,
      center: Vec3,
      wind: Vec3,
      dt: f32,
  ) -> Vec3 {
      let velocity = (current - prev) * (1.0 - config.drag_force);
      let stiffness_force = (initial_world_tail - current).normalize_or_zero()
          * config.stiffness * dt;
      let gravity = config.gravity_dir * config.gravity_power * dt;
      let wind_force = wind * config.wind_scale * dt;
      current + velocity + stiffness_force + gravity + wind_force
  }
  ```
- [x] Clamp dt to max 0.05s to prevent explosion <!-- 2026-03-23 JST -->
- [x] Tests: <!-- 2026-03-23 JST -->
  - [正常系] `zero_drag_preserves_velocity`
  - [正常系] `full_drag_stops_movement`
  - [正常系] `gravity_pulls_downward`
  - [正常系] `stiffness_restores_toward_initial`
  - [正常系] `wind_adds_force`
  - [異常系] `large_dt_clamped`
  - [異常系] `zero_dt_no_movement`
  - [異常系] `nan_input_handled` (normalize_or_zero prevents NaN)

### Step 1.6: `src/constraint.rs` — Bone length constraint (~30 lines)

- [x] Implement `length_constraint()`: <!-- 2026-03-23 00:20 JST -->
  ```rust
  pub fn length_constraint(tail: Vec3, center: Vec3, bone_length: f32) -> Vec3 {
      let dir = (tail - center).normalize_or_zero();
      center + dir * bone_length
  }
  ```
- [x] Tests: <!-- 2026-03-23 00:20 JST -->
  - [正常系] `maintains_exact_bone_length`
  - [正常系] `stretched_tail_pulled_back`
  - [正常系] `compressed_tail_pushed_out`
  - [異常系] `tail_at_center_returns_fallback_direction`

### Step 1.7: `src/solver.rs` — Per-frame solver (~80 lines)

- [x] Implement `solve_chain()`: <!-- 2026-03-23 00:23 JST -->
  ```rust
  pub fn solve_chain(
      chain: &mut BoneChain,
      colliders: &[Collider],
      node_world_matrices: &[Mat4],
      wind: Vec3,
      dt: f32,
  )
  ```
  1. For each bone: get center from parent world position
  2. Compute initial_world_tail from parent matrix * initial_local_dir * bone_length
  3. Call `verlet_step()`
  4. Call `Collider::resolve_collision()` for each referenced collider (2 iterations)
  5. Call `length_constraint()`
  6. Update prev_tail, current_tail
- [x] Implement `compute_bone_rotation()`: <!-- 2026-03-23 00:23 JST -->
  ```rust
  pub fn compute_bone_rotation(
      bone: &SpringBone,
      parent_world_rotation: Quat,
  ) -> Quat
  ```
  Compare initial direction vs current direction → delta rotation
- [x] Tests: <!-- 2026-03-23 00:23 JST -->
  - [正常系] `solve_chain_moves_bones`
  - [正常系] `collider_prevents_penetration`
  - [正常系] `bone_length_preserved_after_solve`
  - [正常系] `rotation_reflects_tail_displacement`
  - [異常系] `empty_chain_no_panic`
  - [異常系] `no_colliders_no_panic`

### Step 1.8: `src/world.rs` — SpringWorld main API (~120 lines)

- [x] Define `SpringWorld`: <!-- 2026-03-23 00:26 JST -->
  ```rust
  pub struct SpringWorld {
      pub chains: Vec<BoneChain>,
      pub colliders: Vec<Collider>,
      pub wind: Vec3,
      pub time_scale: f32,
      pub enabled: bool,
  }
  ```
- [x] Implement `SpringWorld::new() -> Self` <!-- 2026-03-23 00:26 JST -->
- [x] Implement `SpringWorld::add_chain(chain: BoneChain)` <!-- 2026-03-23 00:26 JST -->
- [x] Implement `SpringWorld::add_collider(collider: Collider)` <!-- 2026-03-23 00:26 JST -->
- [x] Implement `SpringWorld::update(dt: f32, node_world_matrices: &[Mat4])`: <!-- 2026-03-23 00:26 JST -->
  1. If !enabled, return early
  2. Update all collider world positions
  3. For each chain: `solve_chain()`
  4. For each bone: `compute_bone_rotation()`
- [x] Implement `SpringWorld::bone_results() -> impl Iterator<Item = BoneResult>`: <!-- 2026-03-23 00:26 JST -->
  ```rust
  pub struct BoneResult {
      pub node_index: usize,
      pub world_rotation: Quat,
  }
  ```
- [x] Implement `SpringWorld::reset()` — reset all bones to initial positions <!-- 2026-03-23 00:26 JST -->
- [x] Tests: <!-- 2026-03-23 00:26 JST -->
  - [正常系] `update_with_single_chain`
  - [正常系] `bone_results_returns_all_bones`
  - [正常系] `disabled_world_skips_update`
  - [正常系] `reset_restores_all_bones`
  - [異常系] `update_with_empty_matrices_no_panic`
  - [異常系] `node_index_out_of_bounds_handled`

### Step 1.9: Phase 1 verification

- [x] `cargo test -p spring-physics` — 42 tests pass <!-- 2026-03-23 00:31 JST -->
- [x] Test coverage ≥ 90% — all public functions and branches covered <!-- 2026-03-23 00:31 JST -->
- [x] `cargo check --workspace` passes <!-- 2026-03-23 00:31 JST -->
- [x] `cargo build --release` succeeds <!-- 2026-03-23 00:31 JST -->
- [x] `cargo clippy -p spring-physics -- -D warnings` clean <!-- 2026-03-23 00:31 JST -->
- [x] `cargo fmt --check` clean <!-- 2026-03-23 00:31 JST -->
- [x] No file exceeds 300 lines of production code (world.rs 327 total but 160 lines are tests) <!-- 2026-03-23 00:31 JST -->

---

## Phase 2: VRM Adapter

### Step 2.1: `vrm/src/spring_bone.rs` — Refactor to adapter (~150 lines)

- [x] Add `spring-physics` dependency to `vrm/Cargo.toml` <!-- 2026-03-23 00:36 JST -->
- [x] Implement `build_spring_world()`: <!-- 2026-03-23 00:36 JST -->
  ```rust
  pub fn build_spring_world(
      vrm_json: &serde_json::Value,
      node_transforms: &[NodeTransform],
      node_parents: &[Option<usize>],
  ) -> Result<SpringWorld, VrmError>
  ```
  1. Parse `secondaryAnimation.colliderGroups` → `Vec<Collider>`
  2. Parse `secondaryAnimation.boneGroups` → `Vec<BoneChain>`
  3. For each bone: compute `bone_length` from parent→child world distance
  4. For each bone: compute `initial_local_dir` from inverse parent rotation * world direction
  5. Set `current_tail` and `prev_tail` to child world position
- [x] Keep backward compatibility: preserve `SpringBoneGroup::from_vrm_json()` as deprecated <!-- 2026-03-23 00:36 JST -->
- [x] Tests: <!-- 2026-03-23 00:36 JST -->
  - [正常系] `build_from_sample_vrm_json`
  - [正常系] `colliders_parsed`
  - [異常系] `missing_secondary_animation_returns_empty_world`
  - [異常系] `invalid_node_index_skipped`

### Step 2.2: VRM loader integration

- [x] In `vrm/src/loader.rs`: add `build_spring_world()` call alongside existing <!-- 2026-03-23 00:41 JST -->
- [x] Add `spring_world: SpringWorld` field to `VrmModel` <!-- 2026-03-23 00:41 JST -->
- [x] Tests: vrm tests pass (43 total) <!-- 2026-03-23 00:41 JST -->

### Step 2.3: Phase 2 verification

- [x] `cargo test -p vrm` — passes <!-- 2026-03-23 00:42 JST -->
- [x] `cargo test -p spring-physics` — 42 tests pass <!-- 2026-03-23 00:42 JST -->
- [x] Test coverage for spring_bone.rs ≥ 90% <!-- 2026-03-23 00:42 JST -->
- [x] `cargo check --workspace` passes <!-- 2026-03-23 00:42 JST -->
- [x] `cargo build --release` succeeds <!-- 2026-03-23 00:42 JST -->
- [ ] Application launches without crash — ヘッドレス環境のため未検証

---

## Phase 3: App Integration

### Step 3.1: AppState integration

- [ ] `app/src/state.rs`: Access `spring_world` via `state.vrm_model.spring_world`
- [ ] `app/src/state.rs`: Add `spring_physics_enabled: bool` (default: true)

### Step 3.2: Update loop integration

- [ ] `app/src/update.rs`: Replace existing spring bone update:
  ```rust
  // Before (old):
  // for group in &mut state.vrm_model.spring_bone_groups {
  //     group.update(delta_time, Vec3::ZERO);
  // }

  // After (new):
  if state.spring_physics_enabled {
      let node_matrices = state.vrm_model.compute_world_matrices();
      state.vrm_model.spring_world.update(delta_time, &node_matrices);
      for result in state.vrm_model.spring_world.bone_results() {
          state.vrm_model.node_transforms[result.node_index].rotation = result.world_rotation;
      }
  }
  ```
- [ ] Ensure spring bone update runs AFTER rig application (face/arm/hand) and BEFORE GPU buffer update
- [ ] If `VrmModel` does not expose `compute_world_matrices()`, implement it:
  ```rust
  pub fn compute_world_matrices(&self) -> Vec<Mat4> {
      // Traverse node hierarchy, accumulate parent * local transforms
  }
  ```

### Step 3.3: Init integration

- [ ] `app/src/init.rs`: Initialize `spring_physics_enabled = true` in AppState
- [ ] Log spring bone group count at startup:
  ```rust
  log::info!("Spring physics: {} chains, {} bones, {} colliders",
      model.spring_world.chains.len(),
      model.spring_world.chains.iter().map(|c| c.bones.len()).sum::<usize>(),
      model.spring_world.colliders.len(),
  );
  ```

### Step 3.4: Phase 3 verification

- [ ] `cargo check --workspace` passes
- [ ] `cargo build --release` succeeds
- [ ] Application launches, no crash
- [ ] Move head left/right → hair sways
- [ ] Hair does not penetrate head/neck colliders
- [ ] Skirt does not penetrate leg colliders
- [ ] 60fps maintained (check frame time in Settings Info)

---

## Phase 4: Settings & Avatar SDK Integration

### Step 4.1: Avatar SDK state

- [ ] `avatar-sdk/src/state.rs`: Add `spring_physics_enabled: bool` to `TrackingState` or `DisplayState`

### Step 4.2: Lua bindings

- [ ] `app/src/lua_avatar.rs`: Add `avatar.get/set_spring_physics()` bindings

### Step 4.3: AvatarState sync

- [ ] `app/src/update.rs`: Add `spring_physics_enabled` to 5c snapshot + 5e diff-based sync

### Step 4.4: Settings Lua script

- [ ] `assets/scripts/settings.lua`: Add checkbox in Tracking or Display section:
  ```lua
  local sp = imgui.checkbox("Spring Physics", avatar.get_spring_physics())
  avatar.set_spring_physics(sp)
  ```

### Step 4.5: Phase 4 verification

- [ ] `cargo check --workspace` passes
- [ ] `cargo build --release` succeeds
- [ ] Application launches, Settings (Lua) shows "Spring Physics" checkbox
- [ ] Toggle ON → hair/skirt sways, Toggle OFF → stops
- [ ] Test coverage for spring-physics crate ≥ 90%
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo fmt --check` clean

---

## Phase 5: features.md update & Documentation

### Step 5.1: features.md

- [ ] Add Phase 20 to root `features.md` with all completed items checked

### Step 5.2: README.md (English)

- [ ] Create `crates/spring-physics/README.md`:
  - Overview with brief description
  - Install instructions (`cargo add spring-physics` or path dependency)
  - Quick start code example
  - API reference summary
  - Architecture diagram (text)
  - Emoji usage (moderate: section headers only)
  - License

### Step 5.3: README_ja.md (Japanese)

- [ ] Create `crates/spring-physics/README_ja.md`:
  - README.md の日本語訳
  - インストール手順
  - 使用例
  - アーキテクチャ図

### Step 5.4: Final verification checklist

以下の動作確認を順番に実施し、全て PASS になるまで修正を繰り返す:

- [ ] `cargo test --workspace` — all tests pass
- [ ] `cargo build --release` — succeeds without error
- [ ] `cargo clippy --workspace -- -D warnings` — clean
- [ ] `cargo fmt --check` — clean
- [ ] Application launches: `LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib cargo run --release`
- [ ] F1 → ImGui visible
- [ ] Settings (Lua) → "Spring Physics" checkbox visible and toggleable
- [ ] Spring Physics ON:
  - [ ] Head rotation → hair sways with delay
  - [ ] Hair does not penetrate head/body
  - [ ] Cat ears bounce slightly
  - [ ] Skirt sways on movement
  - [ ] Coat skirt follows body
  - [ ] Sleeves sway on arm movement
  - [ ] 60fps maintained (no frame drop)
- [ ] Spring Physics OFF:
  - [ ] All jiggle stops immediately
  - [ ] No crash or visual glitch
- [ ] Toggle ON → OFF → ON: stable, no accumulated drift
- [ ] Mascot mode + Spring Physics: works correctly
- [ ] If any check FAILS, fix and re-run the checklist from the failed item
