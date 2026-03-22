use glam::Vec3;
use spring_physics::bone::{BoneChain, SpringBone as PhysSpringBone};
use spring_physics::collider::{Collider as PhysCollider, ColliderShape};
use spring_physics::config::SpringConfig;
use spring_physics::SpringWorld;

use crate::error::VrmError;

// ---------------------------------------------------------------------------
// Legacy structs — kept for backward compatibility until the app is migrated.
// ---------------------------------------------------------------------------

/// Sphere collider for SpringBone collision detection.
#[deprecated(note = "Use spring_physics::collider::Collider via build_spring_world() instead")]
#[derive(Debug, Clone)]
pub struct Collider {
    pub offset: Vec3,
    pub radius: f32,
    /// glTF node index this collider is attached to.
    pub node_index: usize,
}

/// A single spring bone joint with Verlet integration physics.
#[deprecated(note = "Use spring_physics::bone::SpringBone via build_spring_world() instead")]
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

#[allow(deprecated)]
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
#[deprecated(
    note = "Use spring_physics::bone::BoneChain via build_spring_world() instead"
)]
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

#[allow(deprecated)]
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

// ---------------------------------------------------------------------------
// New adapter: VRM JSON → spring_physics::SpringWorld
// ---------------------------------------------------------------------------

/// Helper to parse a Vec3 from a JSON object with "x", "y", "z" fields.
fn parse_vec3(value: &serde_json::Value, default: Vec3) -> Vec3 {
    Vec3::new(
        value
            .get("x")
            .and_then(|v| v.as_f64())
            .unwrap_or(default.x as f64) as f32,
        value
            .get("y")
            .and_then(|v| v.as_f64())
            .unwrap_or(default.y as f64) as f32,
        value
            .get("z")
            .and_then(|v| v.as_f64())
            .unwrap_or(default.z as f64) as f32,
    )
}

/// Build a [`SpringWorld`] from VRM `secondaryAnimation` JSON.
///
/// `node_world_positions` should contain the world-space position of each glTF
/// node.  `node_parents` maps each node index to its parent (`None` for root
/// nodes).
///
/// Bones or colliders that reference node indices beyond the length of
/// `node_world_positions` are silently skipped.
///
/// If `secondaryAnimation` is missing from the VRM extension JSON, an empty
/// `SpringWorld` is returned (no error).
pub fn build_spring_world(
    vrm_ext: &serde_json::Value,
    node_world_positions: &[Vec3],
    node_parents: &[Option<usize>],
) -> Result<SpringWorld, VrmError> {
    let secondary = match vrm_ext.get("secondaryAnimation") {
        Some(s) => s,
        None => return Ok(SpringWorld::new()),
    };

    let mut world = SpringWorld::new();

    // ---- Parse colliderGroups → Vec<PhysCollider> ----
    // We collect them into a Vec<Vec<usize>> so bone groups can reference them
    // by colliderGroup index.
    let mut all_colliders: Vec<PhysCollider> = Vec::new();
    // collider_group_ranges[i] = (start, end) index into all_colliders
    let mut collider_group_ranges: Vec<(usize, usize)> = Vec::new();

    if let Some(collider_groups_json) = secondary.get("colliderGroups").and_then(|c| c.as_array())
    {
        for group in collider_groups_json {
            let node_index = group.get("node").and_then(|n| n.as_u64()).unwrap_or(0) as usize;

            // Skip collider groups whose node is out of bounds.
            if node_index >= node_world_positions.len() {
                collider_group_ranges.push((all_colliders.len(), all_colliders.len()));
                continue;
            }

            let start = all_colliders.len();

            if let Some(colliders_arr) = group.get("colliders").and_then(|c| c.as_array()) {
                for c in colliders_arr {
                    let offset = c
                        .get("offset")
                        .map(|o| parse_vec3(o, Vec3::ZERO))
                        .unwrap_or(Vec3::ZERO);
                    let radius =
                        c.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

                    all_colliders.push(PhysCollider {
                        shape: ColliderShape::Sphere { radius },
                        offset,
                        node_index,
                        world_position: node_world_positions[node_index] + offset,
                    });
                }
            }

            collider_group_ranges.push((start, all_colliders.len()));
        }
    }

    // Add all colliders to the world.
    for collider in &all_colliders {
        world.add_collider(collider.clone());
    }

    // ---- Parse boneGroups → Vec<BoneChain> ----
    if let Some(bone_groups_json) = secondary.get("boneGroups").and_then(|b| b.as_array()) {
        for group_json in bone_groups_json {
            // VRM spec uses "stiffiness" (intentional typo in VRM 0.x).
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
                .map(|d| parse_vec3(d, Vec3::new(0.0, -1.0, 0.0)))
                .unwrap_or(Vec3::new(0.0, -1.0, 0.0));
            let drag_force = group_json
                .get("dragForce")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.4) as f32;
            let hit_radius = group_json
                .get("hitRadius")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.02) as f32;

            let mut config = SpringConfig {
                stiffness,
                gravity_power,
                gravity_dir,
                drag_force,
                hit_radius,
                wind_scale: 0.0,
            };
            config.validate();

            // Bone node indices
            let bone_nodes: Vec<usize> = group_json
                .get("bones")
                .and_then(|b| b.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default();

            let mut bones: Vec<PhysSpringBone> = Vec::new();
            for &node_idx in &bone_nodes {
                // Skip bones whose node index is out of bounds.
                if node_idx >= node_world_positions.len() {
                    log::warn!(
                        "build_spring_world: bone node index {} out of bounds ({}), skipped",
                        node_idx,
                        node_world_positions.len()
                    );
                    continue;
                }

                let child_pos = node_world_positions[node_idx];
                let parent_index = if node_idx < node_parents.len() {
                    node_parents[node_idx]
                } else {
                    None
                };

                let parent_pos = parent_index
                    .and_then(|pi| node_world_positions.get(pi).copied())
                    .unwrap_or(Vec3::ZERO);

                let bone_length = (child_pos - parent_pos).length().max(1e-4);
                let initial_local_dir = (child_pos - parent_pos).normalize_or_zero();

                // If normalize produced zero (coincident positions), default to -Y.
                let initial_local_dir = if initial_local_dir == Vec3::ZERO {
                    Vec3::new(0.0, -1.0, 0.0)
                } else {
                    initial_local_dir
                };

                bones.push(PhysSpringBone::new(
                    node_idx,
                    parent_index,
                    bone_length,
                    initial_local_dir,
                    child_pos, // current_tail = prev_tail = child world pos
                ));
            }

            if bones.is_empty() {
                continue;
            }

            // Gather collider indices that this chain references.
            let collider_group_refs: Vec<usize> = group_json
                .get("colliderGroups")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default();

            let mut chain = BoneChain::new(bones, config);
            for &cg_idx in &collider_group_refs {
                if let Some(&(start, end)) = collider_group_ranges.get(cg_idx) {
                    for idx in start..end {
                        chain.collider_indices.push(idx);
                    }
                }
            }

            world.add_chain(chain);
        }
    }

    Ok(world)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Legacy struct tests (kept to ensure backward compat)
    // -----------------------------------------------------------------------

    #[allow(deprecated)]
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
    #[allow(deprecated)]
    fn update_moves_position() {
        let mut bone = test_bone();
        bone.current_tail = Vec3::new(0.05, -0.08, 0.0);
        bone.prev_tail = Vec3::new(0.05, -0.08, 0.0);
        let initial = bone.current_tail;
        bone.update(0.016, Vec3::ZERO, &[]);
        assert_ne!(bone.current_tail, initial);
    }

    #[test]
    #[allow(deprecated)]
    fn zero_stiffness_falls_with_gravity() {
        let mut bone = test_bone();
        bone.stiffness = 0.0;
        bone.current_tail = Vec3::new(0.1, 0.0, 0.0);
        bone.prev_tail = Vec3::new(0.1, 0.0, 0.0);
        bone.update(0.016, Vec3::ZERO, &[]);
        assert!(
            bone.current_tail.y < 0.0,
            "Tail should move downward with gravity: y={}",
            bone.current_tail.y
        );
    }

    #[test]
    #[allow(deprecated)]
    fn full_drag_minimal_movement() {
        let mut bone = test_bone();
        bone.drag_force = 1.0;
        bone.gravity_power = 0.0;
        let initial = bone.current_tail;
        bone.update(0.016, Vec3::ZERO, &[]);
        let delta = (bone.current_tail - initial).length();
        assert!(delta < 0.1, "Movement should be small with full drag");
    }

    #[test]
    #[allow(deprecated)]
    fn zero_delta_time_no_panic() {
        let mut bone = test_bone();
        bone.update(0.0, Vec3::ZERO, &[]);
    }

    #[test]
    #[allow(deprecated)]
    fn negative_delta_time_no_panic() {
        let mut bone = test_bone();
        bone.update(-1.0, Vec3::ZERO, &[]);
    }

    #[test]
    #[allow(deprecated)]
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
    #[allow(deprecated)]
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
        assert!(dist >= 0.02, "Bone should be pushed out of collider");
    }

    #[test]
    #[allow(deprecated)]
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
    #[allow(deprecated)]
    fn from_vrm_json_no_secondary_animation() {
        let json: serde_json::Value = serde_json::from_str(r#"{}"#).unwrap();
        let groups = SpringBoneGroup::from_vrm_json(&json).unwrap();
        assert!(groups.is_empty());
    }

    // -----------------------------------------------------------------------
    // New adapter tests: build_spring_world()
    // -----------------------------------------------------------------------

    /// [Normal] Build from sample VRM JSON with 1 boneGroup containing 1 bone.
    #[test]
    fn build_from_sample_vrm_json() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "secondaryAnimation": {
                    "boneGroups": [{
                        "stiffiness": 1.0,
                        "gravityPower": 0.5,
                        "gravityDir": {"x": 0.0, "y": -1.0, "z": 0.0},
                        "dragForce": 0.3,
                        "hitRadius": 0.02,
                        "bones": [2],
                        "colliderGroups": []
                    }],
                    "colliderGroups": []
                }
            }"#,
        )
        .unwrap();

        // Node 0 at origin (root), Node 1 at (0,1,0) (parent of 2), Node 2 at (0,2,0)
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ];
        let parents = vec![None, Some(0), Some(1)];

        let world = build_spring_world(&json, &positions, &parents).unwrap();
        assert_eq!(world.chains.len(), 1, "should have 1 chain");
        assert_eq!(world.chains[0].bones.len(), 1, "chain should have 1 bone");

        let bone = &world.chains[0].bones[0];
        assert_eq!(bone.node_index, 2);
        assert_eq!(bone.parent_index, Some(1));
        // bone_length = distance(parent_pos, child_pos) = distance((0,1,0), (0,2,0)) = 1.0
        assert!(
            (bone.bone_length - 1.0).abs() < 1e-4,
            "bone_length should be ~1.0, got {}",
            bone.bone_length
        );
        // current_tail should be the child world pos
        assert!(
            (bone.current_tail - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-4,
            "current_tail should be at child position"
        );
    }

    /// [Normal] Collider groups are parsed with correct count and radius.
    #[test]
    fn colliders_parsed() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "secondaryAnimation": {
                    "boneGroups": [{
                        "stiffiness": 1.0,
                        "gravityPower": 0.0,
                        "dragForce": 0.4,
                        "hitRadius": 0.02,
                        "bones": [1],
                        "colliderGroups": [0]
                    }],
                    "colliderGroups": [{
                        "node": 0,
                        "colliders": [
                            {"offset": {"x": 0.0, "y": 0.5, "z": 0.0}, "radius": 0.1},
                            {"offset": {"x": 0.0, "y": -0.5, "z": 0.0}, "radius": 0.2}
                        ]
                    }]
                }
            }"#,
        )
        .unwrap();

        let positions = vec![Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)];
        let parents = vec![None, Some(0)];

        let world = build_spring_world(&json, &positions, &parents).unwrap();

        // 2 colliders from the single collider group
        assert_eq!(world.colliders.len(), 2);

        // Check radii
        match world.colliders[0].shape {
            ColliderShape::Sphere { radius } => {
                assert!((radius - 0.1).abs() < 1e-6, "first collider radius should be 0.1");
            }
        }
        match world.colliders[1].shape {
            ColliderShape::Sphere { radius } => {
                assert!((radius - 0.2).abs() < 1e-6, "second collider radius should be 0.2");
            }
        }

        // The chain should reference both colliders
        assert_eq!(world.chains[0].collider_indices.len(), 2);
    }

    /// [Abnormal] Missing secondaryAnimation returns an empty SpringWorld.
    #[test]
    fn missing_secondary_animation_returns_empty_world() {
        let json: serde_json::Value = serde_json::from_str(r#"{"title": "test"}"#).unwrap();
        let world = build_spring_world(&json, &[], &[]).unwrap();
        assert!(world.chains.is_empty());
        assert!(world.colliders.is_empty());
    }

    /// [Abnormal] Bone references a node index beyond node_world_positions; it is skipped.
    #[test]
    fn invalid_node_index_skipped() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "secondaryAnimation": {
                    "boneGroups": [{
                        "stiffiness": 1.0,
                        "gravityPower": 0.0,
                        "dragForce": 0.4,
                        "hitRadius": 0.02,
                        "bones": [1, 99],
                        "colliderGroups": []
                    }],
                    "colliderGroups": []
                }
            }"#,
        )
        .unwrap();

        // Only 2 nodes available (indices 0 and 1); node 99 is out of bounds.
        let positions = vec![Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)];
        let parents = vec![None, Some(0)];

        let world = build_spring_world(&json, &positions, &parents).unwrap();

        // Only node 1 should be present; node 99 is skipped.
        assert_eq!(world.chains.len(), 1);
        assert_eq!(
            world.chains[0].bones.len(),
            1,
            "bone with out-of-bounds index should be skipped"
        );
        assert_eq!(world.chains[0].bones[0].node_index, 1);
    }
}
