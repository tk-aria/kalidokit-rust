use glam::{Quat, Vec3};

use crate::config::SpringConfig;

/// A single spring bone joint that tracks a tail position in world space
/// and computes a resulting rotation from physics simulation.
#[derive(Debug, Clone)]
pub struct SpringBone {
    /// Index of this bone's node in the skeleton.
    pub node_index: usize,
    /// Index of the parent node (None for root bones).
    pub parent_index: Option<usize>,
    /// Length of this bone segment.
    pub bone_length: f32,
    /// Bind-pose parent-to-child direction in local space.
    pub initial_local_dir: Vec3,
    /// Current world-space position of the bone tail.
    pub current_tail: Vec3,
    /// Previous frame world-space position of the bone tail.
    pub prev_tail: Vec3,
    /// Computed rotation output from physics simulation.
    pub world_rotation: Quat,
}

impl SpringBone {
    /// Create a new spring bone.
    ///
    /// `world_tail_pos` is used to initialise both `current_tail` and
    /// `prev_tail` so the bone starts at rest.
    pub fn new(
        node_index: usize,
        parent_index: Option<usize>,
        bone_length: f32,
        initial_local_dir: Vec3,
        world_tail_pos: Vec3,
    ) -> Self {
        Self {
            node_index,
            parent_index,
            bone_length,
            initial_local_dir,
            current_tail: world_tail_pos,
            prev_tail: world_tail_pos,
            world_rotation: Quat::IDENTITY,
        }
    }

    /// Reset the bone to a given world-space tail position, clearing
    /// all velocity (prev_tail == current_tail) and rotation.
    pub fn reset(&mut self, world_tail_pos: Vec3) {
        self.current_tail = world_tail_pos;
        self.prev_tail = world_tail_pos;
        self.world_rotation = Quat::IDENTITY;
    }
}

/// A chain of spring bones sharing the same physics configuration.
#[derive(Debug, Clone)]
pub struct BoneChain {
    /// Ordered list of spring bones in this chain (root to tip).
    pub bones: Vec<SpringBone>,
    /// Shared spring physics configuration for this chain.
    pub config: SpringConfig,
    /// Indices into the collider list that affect this chain.
    pub collider_indices: Vec<usize>,
}

impl BoneChain {
    /// Create a new bone chain with the given config and no colliders.
    pub fn new(bones: Vec<SpringBone>, config: SpringConfig) -> Self {
        Self {
            bones,
            config,
            collider_indices: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_initializes_tail_positions() {
        let tail = Vec3::new(1.0, 2.0, 3.0);
        let bone = SpringBone::new(0, None, 0.5, Vec3::Y, tail);

        assert_eq!(bone.current_tail, tail);
        assert_eq!(bone.prev_tail, tail);
        assert_eq!(bone.world_rotation, Quat::IDENTITY);
        assert_eq!(bone.node_index, 0);
        assert_eq!(bone.parent_index, None);
        assert_eq!(bone.bone_length, 0.5);
        assert_eq!(bone.initial_local_dir, Vec3::Y);
    }

    #[test]
    fn reset_restores_initial_position() {
        let mut bone = SpringBone::new(1, Some(0), 1.0, Vec3::Y, Vec3::ZERO);

        // Simulate some movement
        bone.current_tail = Vec3::new(5.0, 5.0, 5.0);
        bone.prev_tail = Vec3::new(4.0, 4.0, 4.0);
        bone.world_rotation = Quat::from_rotation_z(1.0);

        let reset_pos = Vec3::new(0.0, 1.0, 0.0);
        bone.reset(reset_pos);

        assert_eq!(bone.current_tail, reset_pos);
        assert_eq!(bone.prev_tail, reset_pos);
        assert_eq!(bone.world_rotation, Quat::IDENTITY);
    }

    #[test]
    fn zero_bone_length_does_not_panic() {
        let bone = SpringBone::new(0, None, 0.0, Vec3::ZERO, Vec3::ZERO);
        assert_eq!(bone.bone_length, 0.0);

        let mut bone2 = bone.clone();
        bone2.reset(Vec3::ONE);
        assert_eq!(bone2.current_tail, Vec3::ONE);
    }
}
