# Step 1.3: SpringBone and BoneChain

## What was implemented

- `crates/spring-physics/src/bone.rs` (~120 lines)
  - `SpringBone` struct: tracks a single spring bone joint with node index, parent index, bone length, initial local direction, current/previous tail positions, and world rotation.
  - `SpringBone::new()`: initialises both `current_tail` and `prev_tail` to the given world position so the bone starts at rest with `Quat::IDENTITY` rotation.
  - `SpringBone::reset()`: restores tail positions and clears rotation, useful when teleporting a character or reinitialising the simulation.
  - `BoneChain` struct: groups a chain of `SpringBone` instances with a shared `SpringConfig` and an optional set of collider indices.
  - `BoneChain::new()`: convenience constructor.

## Tests

| Test name | Category | Result |
|-----------|----------|--------|
| `new_initializes_tail_positions` | Normal | PASS |
| `reset_restores_initial_position` | Normal | PASS |
| `zero_bone_length_does_not_panic` | Edge case | PASS |

## Verification

```
cargo check -p spring-physics   # OK
cargo test  -p spring-physics   # 7 passed (4 config + 3 bone)
```
