use glam::{Mat4, Quat, Vec3};

use crate::bone::BoneChain;
use crate::collider::Collider;
use crate::constraint::length_constraint;
use crate::integrator::verlet_step;

/// Solve one physics step for a bone chain (KawaiiPhysics algorithm).
///
/// For each bone:
/// 1. Update pose_location from current FK matrices
/// 2. Compute velocity from position history (with damping)
/// 3. Apply gravity and wind
/// 4. Apply stiffness (position-based pull toward pose_location)
/// 5. Collider resolution
/// 6. Bone length constraint
pub fn solve_chain(
    chain: &mut BoneChain,
    colliders: &[Collider],
    node_world_matrices: &[Mat4],
    wind: Vec3,
    dt: f32,
) {
    let dt = dt.clamp(0.0, 0.05); // prevent explosion

    for bone in &mut chain.bones {
        // Step 1: Update pose_location from FK (the "rest position" this frame)
        if bone.node_index < node_world_matrices.len() {
            bone.pose_location = node_world_matrices[bone.node_index]
                .transform_point3(Vec3::ZERO);
        }

        // Get parent world position
        let parent_location = if let Some(parent_idx) = bone.parent_index {
            if parent_idx < node_world_matrices.len() {
                node_world_matrices[parent_idx].transform_point3(Vec3::ZERO)
            } else {
                Vec3::ZERO
            }
        } else {
            Vec3::ZERO
        };

        // Step 2: Verlet velocity with damping
        let velocity = (bone.current_tail - bone.prev_tail) * (1.0 - chain.config.drag_force);
        bone.prev_tail = bone.current_tail;

        // Step 3: Apply velocity + gravity + wind
        let gravity = chain.config.gravity_dir * chain.config.gravity_power * dt;
        let wind_force = wind * chain.config.wind_scale * dt;
        bone.current_tail = bone.current_tail + velocity + gravity + wind_force;

        // Step 4: Stiffness — pull toward pose_location (KawaiiPhysics method)
        // VRM stiffness range is 0-4; map to a gentle pull range for visible spring effect.
        // /16 gives: VRM 0.75 → 0.047, VRM 1.5 → 0.094, VRM 4.0 → 0.25
        let normalized_stiffness = (chain.config.stiffness / 16.0).clamp(0.0, 0.5);
        let exponent = dt * 60.0; // normalize to 60fps
        let stiffness_factor = 1.0 - (1.0 - normalized_stiffness).powf(exponent);
        bone.current_tail = bone.current_tail
            + (bone.pose_location - bone.current_tail) * stiffness_factor;

        // Step 5: Collider resolution
        for _ in 0..2 {
            for &collider_idx in &chain.collider_indices {
                if let Some(collider) = colliders.get(collider_idx) {
                    bone.current_tail =
                        collider.resolve_collision(bone.current_tail, chain.config.hit_radius);
                }
            }
        }

        // Step 6: Bone length constraint — maintain distance from parent
        bone.current_tail =
            length_constraint(bone.current_tail, parent_location, bone.bone_length);
    }
}

/// Compute the rotation for the PARENT bone based on child displacement.
///
/// KawaiiPhysics algorithm:
/// 1. PoseVector = child.pose_location - parent_location (FK rest direction)
/// 2. SimVector = child.current_tail - parent_location (physics direction)
/// 3. DeltaRotation = FindBetweenVectors(PoseVector, SimVector)
/// 4. Result = DeltaRotation * parent.pose_rotation
///
/// The result is the new **component-space rotation** for the parent bone.
pub fn compute_parent_rotation(
    child_pose_location: Vec3,
    child_current_tail: Vec3,
    parent_location: Vec3,
    parent_pose_rotation: Quat,
) -> Quat {
    let pose_vector = (child_pose_location - parent_location).normalize_or_zero();
    let sim_vector = (child_current_tail - parent_location).normalize_or_zero();

    if pose_vector.length_squared() < 1e-6 || sim_vector.length_squared() < 1e-6 {
        return parent_pose_rotation;
    }

    if (pose_vector - sim_vector).length_squared() < 1e-8 {
        return parent_pose_rotation; // no displacement
    }

    let delta = Quat::from_rotation_arc(pose_vector, sim_vector);
    delta * parent_pose_rotation
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
        let parent_location = Vec3::ZERO;
        let child_pose = Vec3::new(0.0, 1.0, 0.0); // rest: straight up
        let child_sim = Vec3::new(1.0, 0.5, 0.0);  // displaced to side
        let parent_rot = Quat::IDENTITY;

        let rot = compute_parent_rotation(child_pose, child_sim, parent_location, parent_rot);
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
    fn rotation_identity_when_tail_at_parent() {
        let parent_location = Vec3::ZERO;
        let child_pose = Vec3::new(0.0, 1.0, 0.0);
        let child_sim = parent_location; // sim tail == parent → zero direction
        let parent_rot = Quat::from_rotation_z(0.5);

        let rot = compute_parent_rotation(child_pose, child_sim, parent_location, parent_rot);
        assert!(
            rot.angle_between(parent_rot) < 1e-4,
            "rotation should equal parent rotation when sim tail is at parent"
        );
    }

    #[test]
    fn rotation_identity_when_pose_at_parent() {
        let parent_location = Vec3::ZERO;
        let child_pose = parent_location; // pose == parent → zero pose direction
        let child_sim = Vec3::new(1.0, 0.0, 0.0);
        let parent_rot = Quat::from_rotation_x(0.3);

        let rot = compute_parent_rotation(child_pose, child_sim, parent_location, parent_rot);
        assert!(
            rot.angle_between(parent_rot) < 1e-4,
            "rotation should equal parent rotation when pose is at parent"
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
