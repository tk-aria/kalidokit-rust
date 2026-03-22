# Step 1.5: Verlet Integration — Implementation Report

## Date: 2026-03-23

## What was implemented

`crates/spring-physics/src/integrator.rs` (~60 lines of production code + tests)

### `verlet_step()` function

Performs one Verlet integration step combining:
- **Velocity** (inertia): `(current - prev) * (1 - drag_force)`
- **Stiffness**: restoring force toward initial tail position
- **Gravity**: configurable direction and power
- **Wind**: external wind force

Delta time is clamped to `[0.0, 0.05]` to prevent physics explosion.

## Tests (8 tests, all passing)

| Category | Test | Description |
|----------|------|-------------|
| Normal | `zero_drag_preserves_velocity` | drag=0 preserves full velocity |
| Normal | `full_drag_stops_movement` | drag=1.0 zeroes velocity |
| Normal | `gravity_pulls_downward` | gravity_power=1.0, dir=(0,-1,0) decreases y |
| Normal | `stiffness_restores_toward_initial` | displaced current pulled back toward initial |
| Normal | `wind_adds_force` | wind=(1,0,0), wind_scale=1.0 increases x |
| Abnormal | `large_dt_clamped` | dt=10.0 clamped to MAX_DT, result finite |
| Abnormal | `zero_dt_no_movement` | dt=0.0, only velocity applies (zeroed by drag) |
| Abnormal | `nan_input_handled` | current==initial_world_tail, normalize_or_zero prevents NaN |

## Verification

```
cargo check -p spring-physics  -- OK
cargo test  -p spring-physics  -- 20 passed (12 existing + 8 new)
```
