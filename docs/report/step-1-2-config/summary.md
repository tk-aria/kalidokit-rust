# Step 1.2: SpringConfig with validation and tests

## What was done

- Implemented `crates/spring-physics/src/config.rs`:
  - `SpringConfig` struct with fields: `stiffness`, `gravity_power`, `gravity_dir`, `drag_force`, `hit_radius`, `wind_scale`
  - All fields `pub`, derives `Debug, Clone`
  - `Default` impl with: stiffness=1.0, gravity_power=0.0, gravity_dir=(0,-1,0), drag_force=0.4, hit_radius=0.02, wind_scale=0.0
  - `validate(&mut self)` method clamping each field to its valid range
  - 4 unit tests covering normal and edge cases

## Commands executed

```bash
cargo check -p spring-physics    # OK
cargo test -p spring-physics     # 4 passed, 0 failed
```

## Test results

| Test | Result |
|------|--------|
| default_values_are_valid | PASS |
| validate_clamps_out_of_range | PASS |
| negative_stiffness_clamped_to_zero | PASS |
| drag_force_above_one_clamped | PASS |
