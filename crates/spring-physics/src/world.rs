use glam::{Mat4, Quat, Vec3};

use crate::bone::BoneChain;
use crate::collider::Collider;
use crate::solver;

/// Result of a spring bone physics step — rotation to apply to a node.
///
/// `local_rotation` should be set directly on the node's local rotation
/// (i.e., `node_transforms[node_index].rotation = local_rotation`).
#[derive(Debug, Clone, Copy)]
pub struct BoneResult {
    pub node_index: usize,
    pub local_rotation: Quat,
}

/// Top-level container that owns all spring bone chains and colliders,
/// and drives the per-frame physics update.
#[derive(Debug, Clone)]
pub struct SpringWorld {
    pub chains: Vec<BoneChain>,
    pub colliders: Vec<Collider>,
    pub wind: Vec3,
    pub time_scale: f32,
    pub enabled: bool,
}

impl SpringWorld {
    /// Create an empty spring world with default settings.
    pub fn new() -> Self {
        Self {
            chains: Vec::new(),
            colliders: Vec::new(),
            wind: Vec3::ZERO,
            time_scale: 1.0,
            enabled: true,
        }
    }

    /// Add a bone chain to the world.
    pub fn add_chain(&mut self, chain: BoneChain) {
        self.chains.push(chain);
    }

    /// Add a collider to the world.
    pub fn add_collider(&mut self, collider: Collider) {
        self.colliders.push(collider);
    }

    /// Advance the physics simulation by `dt` seconds.
    ///
    /// `node_world_matrices` must contain world-space transforms for all
    /// skeleton nodes referenced by bones and colliders.
    pub fn update(&mut self, dt: f32, node_world_matrices: &[Mat4]) {
        if !self.enabled {
            return;
        }

        let effective_dt = dt * self.time_scale;

        // Update collider world positions from skeleton matrices.
        for collider in &mut self.colliders {
            if collider.node_index < node_world_matrices.len() {
                collider.update_world_position(&node_world_matrices[collider.node_index]);
            }
        }

        // Solve each chain.
        for chain in &mut self.chains {
            solver::solve_chain(
                chain,
                &self.colliders,
                node_world_matrices,
                self.wind,
                effective_dt,
            );

            // Compute world rotation for each bone in the chain.
            for bone in &mut chain.bones {
                let center = if let Some(parent_idx) = bone.parent_index {
                    if parent_idx < node_world_matrices.len() {
                        node_world_matrices[parent_idx].transform_point3(Vec3::ZERO)
                    } else {
                        Vec3::ZERO
                    }
                } else {
                    Vec3::ZERO
                };

                let parent_world_rotation = if let Some(parent_idx) = bone.parent_index {
                    if parent_idx < node_world_matrices.len() {
                        let (_, rot, _) =
                            node_world_matrices[parent_idx].to_scale_rotation_translation();
                        rot
                    } else {
                        Quat::IDENTITY
                    }
                } else {
                    Quat::IDENTITY
                };

                let world_rotation = solver::compute_bone_rotation(
                    bone.initial_local_dir,
                    bone.current_tail,
                    center,
                    parent_world_rotation,
                );
                // Convert world rotation to local rotation for node_transforms:
                // local = inverse(parent_world) * world
                bone.world_rotation = parent_world_rotation.inverse() * world_rotation;
            }
        }
    }

    /// Collect the computed rotations for all bones across all chains.
    pub fn bone_results(&self) -> Vec<BoneResult> {
        self.chains
            .iter()
            .flat_map(|chain| {
                chain.bones.iter().map(|bone| BoneResult {
                    node_index: bone.node_index,
                    local_rotation: bone.world_rotation, // now stores local rotation
                })
            })
            .collect()
    }

    /// Reset all bones in all chains to their current tail position,
    /// clearing accumulated velocity and rotation.
    pub fn reset(&mut self) {
        for chain in &mut self.chains {
            for bone in &mut chain.bones {
                bone.reset(bone.current_tail);
            }
        }
    }
}

impl Default for SpringWorld {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bone::{BoneChain, SpringBone};
    use crate::collider::ColliderShape;
    use crate::config::SpringConfig;

    fn default_config() -> SpringConfig {
        SpringConfig {
            stiffness: 0.2,
            gravity_power: 2.0,
            gravity_dir: Vec3::new(0.0, -1.0, 0.0),
            drag_force: 0.2,
            hit_radius: 0.02,
            wind_scale: 0.0,
        }
    }

    fn single_chain_world() -> SpringWorld {
        let bone = SpringBone::new(1, Some(0), 1.0, Vec3::X, Vec3::new(1.0, 0.0, 0.0));
        let chain = BoneChain::new(vec![bone], default_config());
        let mut world = SpringWorld::new();
        world.add_chain(chain);
        world
    }

    // --- Normal cases ---

    #[test]
    fn update_with_single_chain() {
        let mut world = single_chain_world();
        let matrices = vec![Mat4::IDENTITY, Mat4::IDENTITY];
        let initial_tail = world.chains[0].bones[0].current_tail;

        for _ in 0..10 {
            world.update(0.016, &matrices);
        }

        let final_tail = world.chains[0].bones[0].current_tail;
        assert!(
            (final_tail - initial_tail).length() > 1e-4,
            "tail should move after update; initial={initial_tail}, final={final_tail}"
        );
    }

    #[test]
    fn bone_results_returns_all_bones() {
        let bone_a = SpringBone::new(1, Some(0), 1.0, Vec3::X, Vec3::X);
        let bone_b = SpringBone::new(2, Some(1), 1.0, Vec3::X, Vec3::new(2.0, 0.0, 0.0));
        let chain1 = BoneChain::new(vec![bone_a, bone_b], default_config());

        let bone_c = SpringBone::new(4, Some(3), 0.5, Vec3::Y, Vec3::Y);
        let chain2 = BoneChain::new(vec![bone_c], default_config());

        let mut world = SpringWorld::new();
        world.add_chain(chain1);
        world.add_chain(chain2);

        let results = world.bone_results();
        assert_eq!(results.len(), 3, "should have 3 bone results total");
        assert_eq!(results[0].node_index, 1);
        assert_eq!(results[1].node_index, 2);
        assert_eq!(results[2].node_index, 4);
    }

    #[test]
    fn disabled_world_skips_update() {
        let mut world = single_chain_world();
        world.enabled = false;
        let matrices = vec![Mat4::IDENTITY, Mat4::IDENTITY];
        let initial_tail = world.chains[0].bones[0].current_tail;

        for _ in 0..10 {
            world.update(0.016, &matrices);
        }

        let final_tail = world.chains[0].bones[0].current_tail;
        assert_eq!(
            final_tail, initial_tail,
            "tail should not move when world is disabled"
        );
    }

    #[test]
    fn reset_restores_all_bones() {
        let mut world = single_chain_world();
        let matrices = vec![Mat4::IDENTITY, Mat4::IDENTITY];

        // Run some steps to move bones
        for _ in 0..10 {
            world.update(0.016, &matrices);
        }

        let moved_tail = world.chains[0].bones[0].current_tail;
        // Tail should have moved
        assert!(
            (moved_tail - Vec3::new(1.0, 0.0, 0.0)).length() > 1e-4,
            "tail should have moved before reset"
        );

        // Reset and verify velocity is cleared
        world.reset();
        let bone = &world.chains[0].bones[0];
        assert_eq!(
            bone.current_tail, bone.prev_tail,
            "after reset, current_tail and prev_tail should be equal (zero velocity)"
        );
        assert_eq!(
            bone.world_rotation,
            Quat::IDENTITY,
            "after reset, world_rotation should be identity"
        );
    }

    #[test]
    fn time_scale_affects_simulation_speed() {
        let mut world_fast = single_chain_world();
        world_fast.time_scale = 2.0;
        let mut world_normal = single_chain_world();
        world_normal.time_scale = 1.0;
        let matrices = vec![Mat4::IDENTITY, Mat4::IDENTITY];

        for _ in 0..10 {
            world_fast.update(0.016, &matrices);
            world_normal.update(0.016, &matrices);
        }

        let fast_displacement =
            (world_fast.chains[0].bones[0].current_tail - Vec3::new(1.0, 0.0, 0.0)).length();
        let normal_displacement =
            (world_normal.chains[0].bones[0].current_tail - Vec3::new(1.0, 0.0, 0.0)).length();
        assert!(
            fast_displacement > normal_displacement,
            "faster time_scale should cause more displacement; fast={fast_displacement}, normal={normal_displacement}"
        );
    }

    #[test]
    fn default_trait_creates_empty_world() {
        let world: SpringWorld = Default::default();
        assert!(world.chains.is_empty());
        assert!(world.colliders.is_empty());
        assert_eq!(world.wind, Vec3::ZERO);
        assert_eq!(world.time_scale, 1.0);
        assert!(world.enabled);
    }

    // --- Edge / error cases ---

    #[test]
    fn update_with_empty_matrices_no_panic() {
        let mut world = single_chain_world();
        // Pass empty matrices — should not panic
        for _ in 0..5 {
            world.update(0.016, &[]);
        }
        let tail = world.chains[0].bones[0].current_tail;
        assert!(tail.x.is_finite());
        assert!(tail.y.is_finite());
        assert!(tail.z.is_finite());
    }

    #[test]
    fn node_index_out_of_bounds_handled() {
        // Bone references node_index=10 and parent_index=9, but matrices only has 2 entries
        let bone = SpringBone::new(10, Some(9), 1.0, Vec3::Y, Vec3::Y);
        let chain = BoneChain::new(vec![bone], default_config());
        let mut world = SpringWorld::new();
        world.add_chain(chain);

        // Also add a collider with an out-of-bounds node_index
        world.add_collider(Collider {
            shape: ColliderShape::Sphere { radius: 0.5 },
            offset: Vec3::ZERO,
            node_index: 99,
            world_position: Vec3::ZERO,
        });

        let matrices = vec![Mat4::IDENTITY, Mat4::IDENTITY];
        // Should not panic
        for _ in 0..5 {
            world.update(0.016, &matrices);
        }

        let results = world.bone_results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].node_index, 10);
    }
}
