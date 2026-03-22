# spring-physics

Secondary motion simulation for VRM spring bones in Rust.

Inspired by [VRM SpringBone](https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_springBone-1.0) and [KawaiiPhysics](https://github.com/pafuhana1213/KawaiiPhysics) for Unreal Engine.

## Overview

`spring-physics` provides a lightweight, dependency-minimal physics engine for simulating secondary motion on skeletal meshes -- hair, cloth, tails, accessories, and other dangling parts. It uses **Verlet integration** with configurable stiffness, gravity, drag, and wind forces, plus sphere collider support to prevent penetration.

Key properties:

- **Pure Rust** with only `glam` as a dependency
- **Frame-rate independent** via dt-based Verlet integration with clamped max timestep
- **VRM-compatible** data model (chains, collider groups, per-chain config)
- **42 unit tests** covering normal, edge, and error cases

## Install

As a workspace crate:

```toml
[dependencies]
spring-physics = { path = "crates/spring-physics" }
```

When published:

```sh
cargo add spring-physics
```

## Quick Start

```rust
use spring_physics::{SpringWorld, BoneResult};
use spring_physics::bone::{SpringBone, BoneChain};
use spring_physics::config::SpringConfig;
use spring_physics::collider::{Collider, ColliderShape};
use glam::{Mat4, Vec3};

// 1. Create a spring world
let mut world = SpringWorld::new();

// 2. Define a bone chain (e.g. hair strand)
let config = SpringConfig {
    stiffness: 1.0,
    gravity_power: 1.0,
    gravity_dir: Vec3::new(0.0, -1.0, 0.0),
    drag_force: 0.4,
    hit_radius: 0.02,
    wind_scale: 0.0,
};

let bones = vec![
    SpringBone::new(2, Some(1), 0.1, Vec3::NEG_Y, Vec3::new(0.0, -0.1, 0.0)),
    SpringBone::new(3, Some(2), 0.1, Vec3::NEG_Y, Vec3::new(0.0, -0.2, 0.0)),
];
let chain = BoneChain::new(bones, config);
world.add_chain(chain);

// 3. Optionally add colliders (e.g. head sphere)
world.add_collider(Collider {
    shape: ColliderShape::Sphere { radius: 0.1 },
    offset: Vec3::ZERO,
    node_index: 0,
    world_position: Vec3::ZERO,
});

// 4. Each frame: update with delta time and skeleton matrices
let node_matrices = vec![Mat4::IDENTITY; 4];
world.update(1.0 / 60.0, &node_matrices);

// 5. Read results and apply to your skeleton
for result in world.bone_results() {
    // result.node_index  -- which skeleton node
    // result.world_rotation -- computed world-space rotation
    println!("node {} -> rotation {:?}", result.node_index, result.world_rotation);
}
```

## Architecture

```
SpringWorld                        (top-level container)
 |
 |-- chains: Vec<BoneChain>        (groups of connected bones)
 |    |-- bones: Vec<SpringBone>   (individual joints with tail positions)
 |    |-- config: SpringConfig     (stiffness, gravity, drag, wind, hit_radius)
 |    +-- collider_indices         (references into world.colliders)
 |
 +-- colliders: Vec<Collider>      (sphere colliders attached to skeleton nodes)

Per-frame update pipeline:
  1. Update collider world positions from skeleton matrices
  2. For each chain, for each bone:
     a. Verlet integration  (integrator.rs)  -- inertia + forces
     b. Collider resolution (collider.rs)    -- push out of spheres
     c. Length constraint    (constraint.rs)  -- preserve bone length
  3. Compute world rotation  (solver.rs)     -- delta from rest pose
  4. Collect BoneResult vec for the caller to apply
```

## Module Overview

| Module | Description |
|--------|-------------|
| `config` | `SpringConfig` -- per-chain physics parameters with validation/clamping |
| `bone` | `SpringBone` (single joint) and `BoneChain` (ordered chain with shared config) |
| `collider` | `Collider` and `ColliderShape::Sphere` -- collision detection and resolution |
| `integrator` | `verlet_step()` -- Verlet integration with stiffness, gravity, drag, wind |
| `constraint` | `length_constraint()` -- maintains bone length from center |
| `solver` | `solve_chain()` and `compute_bone_rotation()` -- per-chain physics step |
| `world` | `SpringWorld` -- top-level API: `update()`, `bone_results()`, `reset()` |

## Testing

```sh
cargo test -p spring-physics
```

## License

Same as the parent project (kalidokit-rust).
