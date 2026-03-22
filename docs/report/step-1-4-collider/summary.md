# Step 1.4: Collider with sphere collision

## Overview

Implemented `crates/spring-physics/src/collider.rs` (~110 lines) providing sphere-based collision detection for spring bone physics.

## Components

### `ColliderShape` enum
- `Sphere { radius: f32 }` — the only shape variant for now.

### `Collider` struct
- `shape`: the collider geometry
- `offset`: local-space offset from the attached node
- `node_index`: index of the node this collider is attached to
- `world_position`: computed world-space position (updated each frame)

### Methods
- `update_world_position(&mut self, node_world_matrix: &Mat4)` — transforms the local offset into world space using the node's world matrix.
- `resolve_collision(&self, tail: Vec3, hit_radius: f32) -> Vec3` — pushes a spring bone tail out of the collider volume if it penetrates. Handles the degenerate case where the tail is at the collider center by pushing in +Y.

## Tests (all passing)

| Test | Type | Description |
|------|------|-------------|
| `sphere_pushes_point_out` | normal | Tail inside sphere is pushed to surface |
| `point_outside_sphere_unchanged` | normal | Tail outside sphere is returned as-is |
| `world_position_transforms_offset` | normal | Offset correctly transformed by translation matrix |
| `zero_radius_no_collision` | edge | Zero-radius collider does not affect points |
| `point_at_center_pushed_out_safely` | edge | Degenerate case: tail at center pushed in +Y |

## Verification

```
cargo check -p spring-physics  # OK
cargo test -p spring-physics   # 12 passed (5 new + 7 existing)
```
