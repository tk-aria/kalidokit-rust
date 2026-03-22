# Step 1.8: SpringWorld main API

## What was implemented

Replaced the stub `pub struct SpringWorld;` in `crates/spring-physics/src/world.rs` with a full implementation (~130 lines of production code + ~130 lines of tests).

### Public types

- **`BoneResult`** — lightweight result struct carrying `node_index` and `world_rotation` (a `Quat`).
- **`SpringWorld`** — top-level container owning chains, colliders, wind vector, time_scale, and enabled flag.

### SpringWorld methods

| Method | Description |
|--------|-------------|
| `new()` | Empty world, wind=ZERO, time_scale=1.0, enabled=true |
| `add_chain(chain)` | Push a `BoneChain` |
| `add_collider(collider)` | Push a `Collider` |
| `update(dt, node_world_matrices)` | Full physics step: update collider positions, solve chains, compute bone rotations |
| `bone_results()` | Collect `Vec<BoneResult>` from all bones in all chains |
| `reset()` | Reset all bones to current position with zero velocity |

### Update loop detail

1. Early return if `!self.enabled`
2. Scale dt by `self.time_scale`
3. Update each collider's world position from skeleton matrices (bounds-checked)
4. For each chain: `solver::solve_chain(...)` (verlet + collision + length constraint)
5. For each bone: `solver::compute_bone_rotation(...)` using parent rotation extracted via `Mat4::to_scale_rotation_translation()`

### Tests (6 new, all passing)

| Test | Category | Description |
|------|----------|-------------|
| `update_with_single_chain` | Normal | Verifies tail moves under gravity after 10 steps |
| `bone_results_returns_all_bones` | Normal | 2 chains (2+1 bones) yields 3 results with correct node indices |
| `disabled_world_skips_update` | Normal | enabled=false keeps tail unchanged |
| `reset_restores_all_bones` | Normal | After update+reset, velocity is zero and rotation is identity |
| `update_with_empty_matrices_no_panic` | Edge | Empty matrix slice produces finite (non-NaN) results |
| `node_index_out_of_bounds_handled` | Edge | Out-of-bounds node/parent indices handled gracefully |

## Verification

```
cargo check -p spring-physics   # OK
cargo test -p spring-physics    # 36 passed (30 existing + 6 new)
```
