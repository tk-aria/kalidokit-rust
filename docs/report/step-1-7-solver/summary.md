# Step 1.7: Physics Solver

## Overview

Implemented `crates/spring-physics/src/solver.rs` with two public functions:

- `solve_chain()` — Runs one physics simulation step for a `BoneChain`. For each bone: computes the parent world center and initial world tail from `node_world_matrices`, performs Verlet integration, resolves collider collisions (2 iterations), and applies the length constraint.
- `compute_bone_rotation()` — Derives a world-space `Quat` rotation from the displacement between the rest-pose direction and the current tail position, using `Quat::from_rotation_arc`.

## Test Results

All 6 solver tests pass (`cargo test -p spring-physics`):

| Test | Category | Description |
|------|----------|-------------|
| `solve_chain_moves_bones` | Normal | Single-bone chain displaced by gravity after multiple steps |
| `collider_prevents_penetration` | Normal | Sphere collider keeps tail outside its volume |
| `bone_length_preserved_after_solve` | Normal | Distance from center to tail equals `bone_length` |
| `rotation_reflects_tail_displacement` | Normal | Displaced tail produces non-identity rotation |
| `empty_chain_no_panic` | Edge | Chain with zero bones runs without panic |
| `no_colliders_no_panic` | Edge | Invalid collider indices with empty collider slice runs safely |

## Design Notes

- Collider resolution uses 2 iterations per step for stability when multiple colliders overlap.
- `colliders.get(collider_idx)` gracefully handles out-of-bounds indices (returns `None`).
- Test uses a horizontal bone (initial direction = +X) so that gravity (-Y) creates perpendicular force that survives the length constraint normalization.
