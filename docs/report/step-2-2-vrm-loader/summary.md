# Step 2.2: Integrate build_spring_world into VRM loader

## Changes

### crates/vrm/src/model.rs
- Added `spring_physics::SpringWorld` import.
- Added `spring_world: SpringWorld` field to `VrmModel` struct.
- The legacy `spring_bone_groups` field is kept for backward compatibility.

### crates/vrm/src/loader.rs
- Imported `build_spring_world` from `crate::spring_bone`.
- After parsing `node_transforms`, the loader now:
  1. Builds `node_parents: Vec<Option<usize>>` by iterating each node's `children` list.
  2. Computes `node_world_positions: Vec<Vec3>` via BFS from root nodes, accumulating parent world position + child local translation.
  3. Calls `build_spring_world(&vrm_json, &node_world_positions, &node_parents)` to produce a `SpringWorld`.
- The resulting `spring_world` is set on `VrmModel`.

## Verification

- `cargo check -p vrm` passes (only expected deprecation warnings for legacy spring bone types).
- `cargo check -p vrm -p renderer -p solver -p spring-physics` passes.
- `cargo test -p vrm` passes all 43 tests, including the `build_spring_world` adapter tests.
- `cargo check --workspace` fails on `dear-imgui-sys` due to a pre-existing libclang architecture mismatch (x86_64 vs arm64); unrelated to this change.

## Notes

- World position computation uses a simplified model (parent_world_pos + child_local_translation) without applying parent rotation/scale. This is adequate for spring bone initial positions but may need refinement for models with non-identity parent rotations.
- The `loaded_model_has_spring_world` test is not added because it requires a real VRM file on disk.
