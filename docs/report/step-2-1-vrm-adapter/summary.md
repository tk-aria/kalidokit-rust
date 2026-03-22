# Step 2.1: VRM Spring Bone Adapter — `build_spring_world()`

## What was done

1. **Added `spring-physics` dependency** to `crates/vrm/Cargo.toml`.

2. **Rewrote `crates/vrm/src/spring_bone.rs`** to serve as an adapter layer:
   - Existing `SpringBone`, `SpringBoneGroup`, and `Collider` structs are preserved with `#[deprecated]` attributes for backward compatibility.
   - New public function `build_spring_world(vrm_ext, node_world_positions, node_parents)` converts VRM `secondaryAnimation` JSON into a `spring_physics::SpringWorld`.

3. **Handles edge cases**:
   - Missing `secondaryAnimation` returns an empty `SpringWorld` (no error).
   - Bone node indices beyond `node_world_positions` length are silently skipped with a log warning.
   - Collider groups with out-of-bounds node indices are skipped.
   - Zero-length bones (coincident parent/child) default to `-Y` direction.

4. **Tests** (all passing):
   - `build_from_sample_vrm_json` — 1 boneGroup with 1 bone, verifies chain count, bone length, tail position.
   - `colliders_parsed` — colliderGroups with 2 colliders, verifies count, radii, and chain collider indices.
   - `missing_secondary_animation_returns_empty_world` — no `secondaryAnimation` key.
   - `invalid_node_index_skipped` — bone references index 99 with only 2 nodes available.
   - 9 legacy tests for the deprecated structs remain and pass.

## Verification

```
$ cargo check -p vrm          # OK (6 expected deprecation warnings from loader.rs/model.rs)
$ cargo test -p vrm -- spring  # 13 tests passed
```

## Files modified

- `crates/vrm/Cargo.toml` — added `spring-physics` dependency
- `crates/vrm/src/spring_bone.rs` — deprecated old structs, added `build_spring_world()`
