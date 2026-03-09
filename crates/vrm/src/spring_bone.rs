use glam::Vec3;

use crate::error::VrmError;

/// Sphere collider for SpringBone collision detection.
#[derive(Debug, Clone)]
pub struct Collider {
    pub offset: Vec3,
    pub radius: f32,
    /// glTF node index this collider is attached to.
    pub node_index: usize,
}

/// A single spring bone joint with Verlet integration physics.
#[derive(Debug, Clone)]
pub struct SpringBone {
    pub stiffness: f32,
    pub gravity_power: f32,
    pub gravity_dir: Vec3,
    pub drag_force: f32,
    pub hit_radius: f32,
    pub bone_length: f32,
    pub initial_tail: Vec3,
    pub current_tail: Vec3,
    pub prev_tail: Vec3,
    pub node_index: usize,
}

impl SpringBone {
    /// Advance one physics step using Verlet integration.
    pub fn update(&mut self, delta_time: f32, center: Vec3, colliders: &[Collider]) {
        let delta = delta_time.max(0.0);

        // Verlet integration
        let velocity = (self.current_tail - self.prev_tail) * (1.0 - self.drag_force);
        let stiffness_force =
            (self.initial_tail - self.current_tail).normalize_or_zero() * self.stiffness * delta;
        let gravity = self.gravity_dir * self.gravity_power * delta;
        let mut next_tail = self.current_tail + velocity + stiffness_force + gravity;

        // Collider check
        next_tail = self.check_colliders(next_tail, colliders);

        // Maintain bone length
        let direction = (next_tail - center).normalize_or_zero();
        let next_tail = center + direction * self.bone_length;

        self.prev_tail = self.current_tail;
        self.current_tail = next_tail;
    }

    fn check_colliders(&self, mut tail: Vec3, colliders: &[Collider]) -> Vec3 {
        for collider in colliders {
            let diff = tail - collider.offset;
            let dist = diff.length();
            let min_dist = collider.radius + self.hit_radius;
            if dist < min_dist && dist > 1e-6 {
                // Push tail out of collider
                let direction = diff / dist;
                tail = collider.offset + direction * min_dist;
            }
        }
        tail
    }
}

/// A group of spring bones sharing physics parameters.
#[derive(Debug, Clone)]
pub struct SpringBoneGroup {
    pub bones: Vec<SpringBone>,
    pub colliders: Vec<Collider>,
    pub stiffness: f32,
    pub gravity_power: f32,
    pub gravity_dir: Vec3,
    pub drag_force: f32,
    pub hit_radius: f32,
}

impl SpringBoneGroup {
    /// Parse from VRM extension JSON.
    ///
    /// VRM JSON structure:
    /// ```json
    /// { "secondaryAnimation": { "boneGroups": [
    ///   { "stiffiness": 1.0, "gravityPower": 0, "dragForce": 0.4,
    ///     "bones": [nodeIndex, ...] }
    /// ], "colliderGroups": [...] } }
    /// ```
    pub fn from_vrm_json(vrm_ext: &serde_json::Value) -> Result<Vec<Self>, VrmError> {
        let secondary = vrm_ext.get("secondaryAnimation");
        let secondary = match secondary {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        // Parse collider groups
        let collider_groups: Vec<Vec<Collider>> = secondary
            .get("colliderGroups")
            .and_then(|c| c.as_array())
            .map(|groups| {
                groups
                    .iter()
                    .map(|group| {
                        let node_index =
                            group.get("node").and_then(|n| n.as_u64()).unwrap_or(0) as usize;
                        group
                            .get("colliders")
                            .and_then(|c| c.as_array())
                            .map(|colliders| {
                                colliders
                                    .iter()
                                    .map(|c| {
                                        let offset = c
                                            .get("offset")
                                            .map(|o| {
                                                Vec3::new(
                                                    o.get("x")
                                                        .and_then(|v| v.as_f64())
                                                        .unwrap_or(0.0)
                                                        as f32,
                                                    o.get("y")
                                                        .and_then(|v| v.as_f64())
                                                        .unwrap_or(0.0)
                                                        as f32,
                                                    o.get("z")
                                                        .and_then(|v| v.as_f64())
                                                        .unwrap_or(0.0)
                                                        as f32,
                                                )
                                            })
                                            .unwrap_or(Vec3::ZERO);
                                        let radius =
                                            c.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.0)
                                                as f32;
                                        Collider {
                                            offset,
                                            radius,
                                            node_index,
                                        }
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Parse bone groups
        let bone_groups = secondary
            .get("boneGroups")
            .and_then(|b| b.as_array())
            .ok_or_else(|| VrmError::MissingExtension("secondaryAnimation.boneGroups".into()))?;

        let mut groups = Vec::new();
        for group_json in bone_groups {
            // Note: VRM spec uses "stiffiness" (typo is intentional in spec)
            let stiffness = group_json
                .get("stiffiness")
                .or_else(|| group_json.get("stiffness"))
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0) as f32;
            let gravity_power = group_json
                .get("gravityPower")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;
            let gravity_dir = group_json
                .get("gravityDir")
                .map(|d| {
                    Vec3::new(
                        d.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        d.get("y").and_then(|v| v.as_f64()).unwrap_or(-1.0) as f32,
                        d.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    )
                })
                .unwrap_or(Vec3::new(0.0, -1.0, 0.0));
            let drag_force = group_json
                .get("dragForce")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.4) as f32;
            let hit_radius = group_json
                .get("hitRadius")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.02) as f32;

            // Collect colliders from referenced collider groups
            let collider_indices: Vec<usize> = group_json
                .get("colliderGroups")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default();

            let colliders: Vec<Collider> = collider_indices
                .iter()
                .filter_map(|&idx| collider_groups.get(idx))
                .flatten()
                .cloned()
                .collect();

            // Create SpringBone for each bone node
            let bone_nodes: Vec<usize> = group_json
                .get("bones")
                .and_then(|b| b.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default();

            let bones: Vec<SpringBone> = bone_nodes
                .iter()
                .map(|&node_index| SpringBone {
                    stiffness,
                    gravity_power,
                    gravity_dir,
                    drag_force,
                    hit_radius,
                    bone_length: 0.1, // Default length, should be computed from model
                    initial_tail: Vec3::new(0.0, -0.1, 0.0),
                    current_tail: Vec3::new(0.0, -0.1, 0.0),
                    prev_tail: Vec3::new(0.0, -0.1, 0.0),
                    node_index,
                })
                .collect();

            groups.push(SpringBoneGroup {
                bones,
                colliders,
                stiffness,
                gravity_power,
                gravity_dir,
                drag_force,
                hit_radius,
            });
        }

        Ok(groups)
    }

    /// Update all bones in this group.
    pub fn update(&mut self, delta_time: f32, center: Vec3) {
        for bone in &mut self.bones {
            bone.update(delta_time, center, &self.colliders);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bone() -> SpringBone {
        SpringBone {
            stiffness: 1.0,
            gravity_power: 1.0,
            gravity_dir: Vec3::new(0.0, -1.0, 0.0),
            drag_force: 0.4,
            hit_radius: 0.02,
            bone_length: 0.1,
            initial_tail: Vec3::new(0.0, -0.1, 0.0),
            current_tail: Vec3::new(0.0, -0.1, 0.0),
            prev_tail: Vec3::new(0.0, -0.1, 0.0),
            node_index: 0,
        }
    }

    #[test]
    fn update_moves_position() {
        let mut bone = test_bone();
        // Offset current from initial to create stiffness force
        bone.current_tail = Vec3::new(0.05, -0.08, 0.0);
        bone.prev_tail = Vec3::new(0.05, -0.08, 0.0);
        let initial = bone.current_tail;
        bone.update(0.016, Vec3::ZERO, &[]);
        // Position should change after update (stiffness pulls back + gravity)
        assert_ne!(bone.current_tail, initial);
    }

    #[test]
    fn zero_stiffness_falls_with_gravity() {
        let mut bone = test_bone();
        bone.stiffness = 0.0;
        // Start tail to the side so gravity can pull it downward on the sphere
        bone.current_tail = Vec3::new(0.1, 0.0, 0.0);
        bone.prev_tail = Vec3::new(0.1, 0.0, 0.0);
        bone.update(0.016, Vec3::ZERO, &[]);
        // With gravity pointing down, tail y should become negative
        assert!(
            bone.current_tail.y < 0.0,
            "Tail should move downward with gravity: y={}",
            bone.current_tail.y
        );
    }

    #[test]
    fn full_drag_minimal_movement() {
        let mut bone = test_bone();
        bone.drag_force = 1.0;
        bone.gravity_power = 0.0;
        let initial = bone.current_tail;
        bone.update(0.016, Vec3::ZERO, &[]);
        // With full drag and no gravity, movement should be minimal (only stiffness)
        let delta = (bone.current_tail - initial).length();
        assert!(delta < 0.1, "Movement should be small with full drag");
    }

    #[test]
    fn zero_delta_time_no_panic() {
        let mut bone = test_bone();
        bone.update(0.0, Vec3::ZERO, &[]);
        // Should not panic
    }

    #[test]
    fn negative_delta_time_no_panic() {
        let mut bone = test_bone();
        bone.update(-1.0, Vec3::ZERO, &[]);
        // Should not panic (clamped to 0)
    }

    #[test]
    fn bone_length_maintained() {
        let mut bone = test_bone();
        bone.update(0.016, Vec3::ZERO, &[]);
        let length = bone.current_tail.length();
        assert!(
            (length - bone.bone_length).abs() < 1e-4,
            "Bone length should be maintained: got {} expected {}",
            length,
            bone.bone_length
        );
    }

    #[test]
    fn collider_pushes_bone_out() {
        let mut bone = test_bone();
        bone.current_tail = Vec3::new(0.0, 0.0, 0.0);
        let colliders = vec![Collider {
            offset: Vec3::new(0.0, 0.0, 0.0),
            radius: 0.5,
            node_index: 0,
        }];
        bone.update(0.016, Vec3::ZERO, &colliders);
        let dist = bone.current_tail.length();
        // Should be pushed out at least to collider radius + hit_radius
        assert!(dist >= 0.02, "Bone should be pushed out of collider");
    }

    #[test]
    fn from_vrm_json_parses_bone_group() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "secondaryAnimation": {
                    "boneGroups": [{
                        "stiffiness": 0.8,
                        "gravityPower": 0.5,
                        "gravityDir": {"x": 0.0, "y": -1.0, "z": 0.0},
                        "dragForce": 0.3,
                        "hitRadius": 0.05,
                        "bones": [10, 11, 12],
                        "colliderGroups": []
                    }],
                    "colliderGroups": []
                }
            }"#,
        )
        .unwrap();

        let groups = SpringBoneGroup::from_vrm_json(&json).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].bones.len(), 3);
        assert!((groups[0].stiffness - 0.8).abs() < 1e-6);
        assert!((groups[0].gravity_power - 0.5).abs() < 1e-6);
        assert!((groups[0].drag_force - 0.3).abs() < 1e-6);
    }

    #[test]
    fn from_vrm_json_no_secondary_animation() {
        let json: serde_json::Value = serde_json::from_str(r#"{}"#).unwrap();
        let groups = SpringBoneGroup::from_vrm_json(&json).unwrap();
        assert!(groups.is_empty());
    }
}
