# Step 3.1-3.3: App Integration -- Spring Physics Update Loop + Init Logging

## Changes

### Step 3.1: AppState integration
- Added `pub spring_physics_enabled: bool` field to `AppState` in `crates/app/src/state.rs`.
- Initialized to `true` in `crates/app/src/init.rs` AppState construction.

### Step 3.2: Update loop integration
- Replaced the old `spring_bone_groups` iteration in `crates/app/src/update.rs` (section 3.5) with the new `spring-physics` crate integration:
  1. Computes world matrices via `VrmModel::compute_world_matrices()`.
  2. Calls `SpringWorld::update(dt, &node_matrices)`.
  3. Applies `bone_results()` back to `node_transforms[].rotation`.
- Gated behind `state.spring_physics_enabled` flag.
- Added `VrmModel::compute_world_matrices()` method in `crates/vrm/src/model.rs` which delegates to `HumanoidBones::compute_joint_matrices()`.

### Step 3.3: Init logging
- Added `log::info!` after VRM model loading that reports chain count, bone count, and collider count from `spring_world`.

## Verification
- `cargo check -p kalidokit-rust` -- PASS
- `cargo build --release` -- PASS (12.98s)

## Notes on compute_world_matrices
- `VrmModel` did not have a `compute_world_matrices()` method. Added as a thin wrapper around the existing `HumanoidBones::compute_joint_matrices(&self.node_transforms)` which already performs BFS-based FK traversal of the full node hierarchy and returns `Vec<Mat4>`.
- The `SpringWorld::update()` API expects `&[Mat4]` of world-space transforms indexed by node index, which is exactly what `compute_joint_matrices` returns.
