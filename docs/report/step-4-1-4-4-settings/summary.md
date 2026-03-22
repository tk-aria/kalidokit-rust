# Phase 4 Steps 4.1-4.4: Spring Physics Settings Integration

## Changes

### 4.1 Avatar SDK state (`crates/avatar-sdk/src/state.rs`)
- Added `spring_physics_enabled: bool` field to `DisplayState`
- Default value: `true`

### 4.2 Lua bindings (`crates/app/src/lua_avatar.rs`)
- Added `avatar.get_spring_physics()` -- returns `DisplayState.spring_physics_enabled`
- Added `avatar.set_spring_physics(v: bool)` -- sets the value
- Follows the existing get/set pattern (clone Arc handle, create_function)

### 4.3 AvatarState sync (`crates/app/src/update.rs`)
- Section 5c (AppState -> AvatarState): syncs `state.spring_physics_enabled` into `av.display.spring_physics_enabled`
- Section 5e (AvatarState -> AppState): compares against snapshot and writes back only if Lua changed the value

### 4.4 Settings Lua script (`assets/scripts/settings.lua`)
- Added "Spring Physics" checkbox in the Display section, after "Avatar on Top"

## Verification
- `cargo check -p kalidokit-rust` -- passes (no new warnings)
- `cargo build --release` -- passes
