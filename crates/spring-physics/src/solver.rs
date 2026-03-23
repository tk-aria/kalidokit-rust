use glam::{Mat4, Quat, Vec3};

use crate::bone::BoneChain;
use crate::collider::Collider;
use crate::constraint::length_constraint;
use crate::integrator::verlet_step;

/// Solve one physics step for a bone chain.
pub fn solve_chain(
    chain: &mut BoneChain,
    colliders: &[Collider],
    node_world_matrices: &[Mat4],
    wind: Vec3,
    dt: f32,
) {
    for bone in &mut chain.bones {
        // Get parent world position (center of rotation)
        let center = if let Some(parent_idx) = bone.parent_index {
            if parent_idx < node_world_matrices.len() {
                node_world_matrices[parent_idx].transform_point3(Vec3::ZERO)
            } else {
                Vec3::ZERO
            }
        } else {
            Vec3::ZERO
        };

        // Compute initial world tail from parent transform + local direction.
        // initial_local_dir is in parent-local space.
        let initial_world_tail = if let Some(parent_idx) = bone.parent_index {
            if parent_idx < node_world_matrices.len() {
                let parent_mat = node_world_matrices[parent_idx];
                parent_mat.transform_point3(bone.initial_local_dir * bone.bone_length)
            } else {
                center + bone.initial_local_dir * bone.bone_length
            }
        } else {
            center + bone.initial_local_dir * bone.bone_length
        };

        // Verlet integration
        let next = verlet_step(
            bone.current_tail,
            bone.prev_tail,
            &chain.config,
            initial_world_tail,
            center,
            wind,
            dt,
        );

        // Collider resolution (2 iterations for stability)
        let mut resolved = next;
        for _ in 0..2 {
            for &collider_idx in &chain.collider_indices {
                if let Some(collider) = colliders.get(collider_idx) {
                    resolved = collider.resolve_collision(resolved, chain.config.hit_radius);
                }
            }
        }

        // Length constraint
        let constrained = length_constraint(resolved, center, bone.bone_length);

        bone.prev_tail = bone.current_tail;
        bone.current_tail = constrained;
    }
}

/// Compute the LOCAL rotation delta for a spring bone.
///
/// Returns a quaternion that, when applied to the node's LOCAL rotation,
/// produces the desired spring bone displacement. This is computed as
/// the rotation from the rest-pose world direction to the current tail direction,
/// transformed into the parent's local space.
pub fn compute_bone_rotation(
    initial_dir: Vec3,
    current_tail: Vec3,
    center: Vec3,
    parent_world_rotation: Quat,
) -> Quat {
    let current_dir = (current_tail - center).normalize_or_zero();
    // initial_dir is in parent-local space; transform to world.
    let initial_world_dir = parent_world_rotation * initial_dir;

    if current_dir.length_squared() < 1e-6 || initial_world_dir.length_squared() < 1e-6 {
        return parent_world_rotation;
    }

    let rotation_delta = Quat::from_rotation_arc(initial_world_dir.normalize(), current_dir);
    rotation_delta * parent_world_rotation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bone::{BoneChain, SpringBone};
    use crate::collider::{Collider, ColliderShape};
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

    fn single_bone_chain() -> BoneChain {
        // Horizontal bone: center at origin, tail at (1,0,0).
        // Gravity (-Y) is perpendicular to the bone, so it creates angular displacement
        // that length_constraint cannot cancel.
        let bone = SpringBone::new(1, Some(0), 1.0, Vec3::X, Vec3::new(1.0, 0.0, 0.0));
        BoneChain::new(vec![bone], default_config())
    }

    #[test]
    fn solve_chain_moves_bones() {
        let mut chain = single_bone_chain();
        let matrices = vec![Mat4::IDENTITY];
        let initial_tail = chain.bones[0].current_tail;

        // Run several steps so forces accumulate
        for _ in 0..10 {
            solve_chain(&mut chain, &[], &matrices, Vec3::ZERO, 0.016);
        }

        let final_tail = chain.bones[0].current_tail;
        // With gravity enabled, the tail should have moved from its initial position
        assert!(
            (final_tail - initial_tail).length() > 1e-4,
            "tail should move due to gravity; initial={initial_tail}, final={final_tail}"
        );
    }

    #[test]
    fn collider_prevents_penetration() {
        let mut chain = single_bone_chain();
        // Place a sphere collider at (0, 0.5, 0) with radius 0.5
        // The bone tail starts at (0, 1, 0) and gravity pulls it down
        let collider = Collider {
            shape: ColliderShape::Sphere { radius: 0.5 },
            offset: Vec3::ZERO,
            node_index: 0,
            world_position: Vec3::new(0.0, 0.3, 0.0),
        };
        chain.collider_indices = vec![0];
        let matrices = vec![Mat4::IDENTITY];

        for _ in 0..50 {
            solve_chain(
                &mut chain,
                &[collider.clone()],
                &matrices,
                Vec3::ZERO,
                0.016,
            );
        }

        let tail = chain.bones[0].current_tail;
        let dist_to_collider = (tail - collider.world_position).length();
        let min_dist = 0.5 + chain.config.hit_radius;
        assert!(
            dist_to_collider >= min_dist - 1e-4,
            "tail should not penetrate collider; dist={dist_to_collider}, min={min_dist}"
        );
    }

    #[test]
    fn bone_length_preserved_after_solve() {
        let mut chain = single_bone_chain();
        let matrices = vec![Mat4::IDENTITY];
        let bone_length = chain.bones[0].bone_length;

        for _ in 0..20 {
            solve_chain(&mut chain, &[], &matrices, Vec3::ZERO, 0.016);
        }

        // Center is at origin (parent matrix is identity)
        let center = Vec3::ZERO;
        let tail = chain.bones[0].current_tail;
        let dist = (tail - center).length();
        assert!(
            (dist - bone_length).abs() < 1e-4,
            "bone length should be preserved; expected={bone_length}, got={dist}"
        );
    }

    #[test]
    fn rotation_reflects_tail_displacement() {
        let initial_dir = Vec3::Y;
        let center = Vec3::ZERO;
        // Displace tail to the side
        let displaced_tail = Vec3::new(1.0, 0.5, 0.0);
        let parent_rot = Quat::IDENTITY;

        let rot = compute_bone_rotation(initial_dir, displaced_tail, center, parent_rot);
        // The rotation should not be identity since the tail is displaced
        let angle = rot.angle_between(Quat::IDENTITY);
        assert!(
            angle > 1e-4,
            "rotation should reflect displacement; angle={angle}"
        );
    }

    #[test]
    fn empty_chain_no_panic() {
        let mut chain = BoneChain::new(vec![], default_config());
        let matrices = vec![Mat4::IDENTITY];
        // Should not panic
        solve_chain(&mut chain, &[], &matrices, Vec3::ZERO, 0.016);
        assert!(chain.bones.is_empty());
    }

    #[test]
    fn rotation_identity_when_tail_at_center() {
        let initial_dir = Vec3::Y;
        let center = Vec3::ZERO;
        let tail = center; // tail == center → current_dir is zero
        let parent_rot = Quat::from_rotation_z(0.5);

        let rot = compute_bone_rotation(initial_dir, tail, center, parent_rot);
        // Should return parent_world_rotation unchanged
        assert!(
            rot.angle_between(parent_rot) < 1e-4,
            "rotation should equal parent rotation when tail is at center"
        );
    }

    #[test]
    fn rotation_identity_when_initial_dir_zero() {
        let initial_dir = Vec3::ZERO; // zero initial direction
        let center = Vec3::ZERO;
        let tail = Vec3::new(1.0, 0.0, 0.0);
        let parent_rot = Quat::from_rotation_x(0.3);

        let rot = compute_bone_rotation(initial_dir, tail, center, parent_rot);
        // initial_world_dir = parent_rot * ZERO = ZERO → early return
        assert!(
            rot.angle_between(parent_rot) < 1e-4,
            "rotation should equal parent rotation when initial_dir is zero"
        );
    }

    #[test]
    fn no_colliders_no_panic() {
        let mut chain = single_bone_chain();
        // Explicitly set collider indices that point to nothing
        chain.collider_indices = vec![0, 1, 2];
        let matrices = vec![Mat4::IDENTITY];
        // Should not panic even with invalid collider indices and empty collider slice
        solve_chain(&mut chain, &[], &matrices, Vec3::ZERO, 0.016);
        // Bone should still be valid
        assert!(chain.bones[0].current_tail.x.is_finite());
        assert!(chain.bones[0].current_tail.y.is_finite());
        assert!(chain.bones[0].current_tail.z.is_finite());
    }
}
