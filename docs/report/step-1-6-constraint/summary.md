# Step 1.6: Bone Length Constraint

## Overview

Implemented `length_constraint()` in `crates/spring-physics/src/constraint.rs`.
This function enforces a fixed bone length between a center joint and its tail,
which is essential for spring-bone physics to prevent bones from stretching or compressing.

## Implementation

- **Function**: `length_constraint(tail: Vec3, center: Vec3, bone_length: f32) -> Vec3`
- **Logic**: Normalizes the direction from center to tail and scales to exact `bone_length`
- **Edge case**: When tail coincides with center (zero-length direction), falls back to +Y direction

## Tests (4 cases, all passing)

| Test | Category | Description |
|------|----------|-------------|
| `maintains_exact_bone_length` | Normal | Result distance equals bone_length exactly |
| `stretched_tail_pulled_back` | Normal | Tail far from center is pulled to exact distance, direction preserved |
| `compressed_tail_pushed_out` | Normal | Tail too close is pushed to exact distance, direction preserved |
| `tail_at_center_returns_fallback_direction` | Edge | Tail == center uses +Y fallback |

## Verification

```
cargo check -p spring-physics   # OK
cargo test -p spring-physics    # 24 passed (including 4 new constraint tests)
```
