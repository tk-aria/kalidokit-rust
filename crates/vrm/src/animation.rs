use std::collections::HashMap;
use std::path::Path;

use glam::Quat;

use crate::bone::HumanoidBoneName;

/// A single animation channel: keyframe times and rotation values for one bone.
///
/// Rotations are stored as **world-space delta rotations**:
/// `delta = mixamo_world_rest_inv * mixamo_world_anim`
///
/// This correctly handles the bone orientation difference between Mixamo and VRM.
pub struct BoneChannel {
    pub bone: HumanoidBoneName,
    /// Keyframe timestamps in seconds.
    pub times: Vec<f32>,
    /// World-space delta quaternion rotations at each keyframe.
    pub rotations: Vec<Quat>,
}

/// A loaded animation clip containing per-bone rotation keyframes.
pub struct AnimationClip {
    pub name: String,
    /// Total duration in seconds.
    pub duration: f32,
    /// Per-bone rotation channels (world-space delta rotations).
    pub channels: Vec<BoneChannel>,
}

impl AnimationClip {
    /// Load an animation clip from a glTF/GLB file and convert to world-space delta rotations.
    ///
    /// Mixamo and VRM skeletons have different bone orientations (e.g., Mixamo arms
    /// have 90° world rotation from parent chain, VRM arms have ~IDENTITY).
    /// Using local deltas (`local_rest_inv * local_anim`) produces wrong results.
    ///
    /// Instead, we compute world-space deltas via FK:
    /// 1. Build full skeleton hierarchy from glTF nodes
    /// 2. For each frame, FK-solve world rotations for all bones
    /// 3. Delta = `world_rest_inv * world_anim`
    ///
    /// The animation player applies: `final = vrm_bind * delta`
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let (document, buffers, _images) = gltf::import(path)?;

        // Build node info: name, rest rotation, parent index
        let node_count = document.nodes().len();
        let mut node_names: Vec<String> = vec![String::new(); node_count];
        let mut node_rest_rot: Vec<Quat> = vec![Quat::IDENTITY; node_count];
        let mut node_parent: Vec<Option<usize>> = vec![None; node_count];

        for node in document.nodes() {
            let idx = node.index();
            node_names[idx] = node.name().unwrap_or("").to_string();
            let (_, rot, _) = node.transform().decomposed();
            node_rest_rot[idx] = Quat::from_xyzw(rot[0], rot[1], rot[2], rot[3]);
            for child in node.children() {
                node_parent[child.index()] = Some(idx);
            }
        }

        // Compute world rest rotations via FK (BFS from roots)
        let world_rest = compute_world_rotations(&node_rest_rot, &node_parent);

        let anim = document
            .animations()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No animations found in {}", path.display()))?;

        // Collect all rotation channels, grouped by node index
        // We need ALL nodes' keyframes to FK-solve each frame
        let mut node_channels: HashMap<usize, (Vec<f32>, Vec<Quat>)> = HashMap::new();
        let mut duration: f32 = 0.0;

        for channel in anim.channels() {
            if channel.target().property() != gltf::animation::Property::Rotation {
                continue;
            }
            let node_idx = channel.target().node().index();
            let reader = channel.reader(|buf| buffers.get(buf.index()).map(|d| d.0.as_slice()));

            let times: Vec<f32> = match reader.read_inputs() {
                Some(iter) => iter.collect(),
                None => continue,
            };
            let rotations: Vec<Quat> = match reader.read_outputs() {
                Some(gltf::animation::util::ReadOutputs::Rotations(rots)) => rots
                    .into_f32()
                    .map(|[x, y, z, w]| Quat::from_xyzw(x, y, z, w))
                    .collect(),
                _ => continue,
            };

            if let Some(&last) = times.last() {
                duration = duration.max(last);
            }
            node_channels.insert(node_idx, (times, rotations));
        }

        // Identify which node indices map to VRM bones
        let mut bone_node_map: Vec<(HumanoidBoneName, usize)> = Vec::new();
        for (idx, name) in node_names.iter().enumerate() {
            if let Some(bone) = mixamo_to_vrm(name) {
                if node_channels.contains_key(&idx) {
                    bone_node_map.push((bone, idx));
                }
            }
        }

        // Get the number of keyframes (assume all channels have same count)
        let num_frames = node_channels
            .values()
            .map(|(t, _)| t.len())
            .max()
            .unwrap_or(0);

        if num_frames == 0 {
            anyhow::bail!("No keyframes found in animation");
        }

        // For each frame, FK-solve world rotations and compute world-space deltas
        let mut channels: Vec<BoneChannel> = bone_node_map
            .iter()
            .map(|(bone, _)| BoneChannel {
                bone: *bone,
                times: Vec::with_capacity(num_frames),
                rotations: Vec::with_capacity(num_frames),
            })
            .collect();

        // Build VRM humanoid bone parent lookup for world→local conversion
        let vrm_bone_parent = build_vrm_bone_parent_map();

        // Build bone→channel index lookup
        let bone_to_ch: HashMap<HumanoidBoneName, usize> = bone_node_map
            .iter()
            .enumerate()
            .map(|(i, (bone, _))| (*bone, i))
            .collect();

        for frame_idx in 0..num_frames {
            // Build per-node local rotations for this frame
            let mut frame_local: Vec<Quat> = node_rest_rot.clone();
            for (&node_idx, (_, rots)) in &node_channels {
                if frame_idx < rots.len() {
                    frame_local[node_idx] = rots[frame_idx];
                }
            }

            // FK-solve world rotations for this frame
            let frame_world = compute_world_rotations(&frame_local, &node_parent);

            // Compute world-space delta for each bone.
            //
            // `world_anim * world_rest_inv` gives the rotation that transforms the
            // rest-pose direction to the animated direction.
            //
            // Coordinate system correction: Mixamo GLB (exported from Blender FBX→GLB)
            // has the model facing -Z, while VRM models face +Z. The difference is a
            // 180° Y rotation. We conjugate the delta by R_y(180°) to convert coordinate
            // systems. For quaternion (x,y,z,w), this negates x and z components.
            let world_deltas: Vec<Quat> = bone_node_map
                .iter()
                .map(|(_, node_idx)| {
                    let world_rest_inv = world_rest[*node_idx].inverse();
                    let world_anim = frame_world[*node_idx];
                    let raw = (world_anim * world_rest_inv).normalize();
                    // Conjugate by 180° Y: (x,y,z,w) → (-x,y,-z,w)
                    Quat::from_xyzw(-raw.x, raw.y, -raw.z, raw.w)
                })
                .collect();

            // Convert world deltas → local deltas using VRM bone hierarchy.
            // local_delta = parent_world_delta_inv * bone_world_delta
            // This accounts for the parent bone's animation when setting child rotations.
            for (ch_idx, (bone, node_idx)) in bone_node_map.iter().enumerate() {
                let world_delta = world_deltas[ch_idx];

                let local_delta = if let Some(parent_bone) = vrm_bone_parent.get(bone) {
                    if let Some(&parent_ch_idx) = bone_to_ch.get(parent_bone) {
                        let parent_world_delta = world_deltas[parent_ch_idx];
                        (parent_world_delta.inverse() * world_delta).normalize()
                    } else {
                        world_delta
                    }
                } else {
                    world_delta
                };

                if let Some((times, _)) = node_channels.get(node_idx) {
                    if frame_idx < times.len() {
                        channels[ch_idx].times.push(times[frame_idx]);
                    }
                }
                channels[ch_idx].rotations.push(local_delta);
            }
        }

        // Remove channels with no keyframes
        channels.retain(|ch| !ch.times.is_empty());

        let name = anim.name().unwrap_or("idle").to_string();

        log::info!(
            "Loaded animation '{}': {:.2}s, {} bone channels (retargeted local delta)",
            name,
            duration,
            channels.len()
        );

        Ok(AnimationClip {
            name,
            duration,
            channels,
        })
    }
}

/// Compute world-space rotations for all nodes via FK.
fn compute_world_rotations(local_rots: &[Quat], parents: &[Option<usize>]) -> Vec<Quat> {
    let n = local_rots.len();
    let mut world = vec![Quat::IDENTITY; n];
    let mut computed = vec![false; n];

    // Topological sort: process parents before children
    // Simple iterative approach: keep processing until all done
    let mut remaining = n;
    while remaining > 0 {
        let mut progress = false;
        for i in 0..n {
            if computed[i] {
                continue;
            }
            match parents[i] {
                None => {
                    // Root node
                    world[i] = local_rots[i];
                    computed[i] = true;
                    remaining -= 1;
                    progress = true;
                }
                Some(parent) if computed[parent] => {
                    world[i] = (world[parent] * local_rots[i]).normalize();
                    computed[i] = true;
                    remaining -= 1;
                    progress = true;
                }
                _ => {}
            }
        }
        if !progress {
            break; // Avoid infinite loop on broken hierarchies
        }
    }

    world
}

/// Build a map of VRM humanoid bone → parent bone.
///
/// This hardcodes the standard VRM humanoid skeleton hierarchy.
fn build_vrm_bone_parent_map() -> HashMap<HumanoidBoneName, HumanoidBoneName> {
    use HumanoidBoneName::*;
    let pairs: &[(HumanoidBoneName, HumanoidBoneName)] = &[
        // Spine chain
        (Spine, Hips),
        (Chest, Spine),
        (UpperChest, Chest),
        (Neck, UpperChest),
        (Head, Neck),
        // Left arm
        (LeftShoulder, UpperChest),
        (LeftUpperArm, LeftShoulder),
        (LeftLowerArm, LeftUpperArm),
        (LeftHand, LeftLowerArm),
        // Right arm
        (RightShoulder, UpperChest),
        (RightUpperArm, RightShoulder),
        (RightLowerArm, RightUpperArm),
        (RightHand, RightLowerArm),
        // Left leg
        (LeftUpperLeg, Hips),
        (LeftLowerLeg, LeftUpperLeg),
        (LeftFoot, LeftLowerLeg),
        (LeftToes, LeftFoot),
        // Right leg
        (RightUpperLeg, Hips),
        (RightLowerLeg, RightUpperLeg),
        (RightFoot, RightLowerLeg),
        (RightToes, RightFoot),
        // Left fingers
        (LeftThumbProximal, LeftHand),
        (LeftThumbIntermediate, LeftThumbProximal),
        (LeftThumbDistal, LeftThumbIntermediate),
        (LeftIndexProximal, LeftHand),
        (LeftIndexIntermediate, LeftIndexProximal),
        (LeftIndexDistal, LeftIndexIntermediate),
        (LeftMiddleProximal, LeftHand),
        (LeftMiddleIntermediate, LeftMiddleProximal),
        (LeftMiddleDistal, LeftMiddleIntermediate),
        (LeftRingProximal, LeftHand),
        (LeftRingIntermediate, LeftRingProximal),
        (LeftRingDistal, LeftRingIntermediate),
        (LeftLittleProximal, LeftHand),
        (LeftLittleIntermediate, LeftLittleProximal),
        (LeftLittleDistal, LeftLittleIntermediate),
        // Right fingers
        (RightThumbProximal, RightHand),
        (RightThumbIntermediate, RightThumbProximal),
        (RightThumbDistal, RightThumbIntermediate),
        (RightIndexProximal, RightHand),
        (RightIndexIntermediate, RightIndexProximal),
        (RightIndexDistal, RightIndexIntermediate),
        (RightMiddleProximal, RightHand),
        (RightMiddleIntermediate, RightMiddleProximal),
        (RightMiddleDistal, RightMiddleIntermediate),
        (RightRingProximal, RightHand),
        (RightRingIntermediate, RightRingProximal),
        (RightRingDistal, RightRingIntermediate),
        (RightLittleProximal, RightHand),
        (RightLittleIntermediate, RightLittleProximal),
        (RightLittleDistal, RightLittleIntermediate),
        // Eyes
        (LeftEye, Head),
        (RightEye, Head),
        (Jaw, Head),
    ];
    pairs.iter().cloned().collect()
}

/// Map a Mixamo bone name (e.g. "mixamorig:Hips") to a VRM HumanoidBoneName.
fn mixamo_to_vrm(name: &str) -> Option<HumanoidBoneName> {
    let bone = name.strip_prefix("mixamorig:").unwrap_or(name);

    match bone {
        "Hips" => Some(HumanoidBoneName::Hips),
        "Spine" => Some(HumanoidBoneName::Spine),
        "Spine1" => Some(HumanoidBoneName::Chest),
        "Spine2" => Some(HumanoidBoneName::UpperChest),
        "Neck" => Some(HumanoidBoneName::Neck),
        "Head" => Some(HumanoidBoneName::Head),
        "LeftShoulder" => Some(HumanoidBoneName::LeftShoulder),
        "LeftArm" => Some(HumanoidBoneName::LeftUpperArm),
        "LeftForeArm" => Some(HumanoidBoneName::LeftLowerArm),
        "LeftHand" => Some(HumanoidBoneName::LeftHand),
        "RightShoulder" => Some(HumanoidBoneName::RightShoulder),
        "RightArm" => Some(HumanoidBoneName::RightUpperArm),
        "RightForeArm" => Some(HumanoidBoneName::RightLowerArm),
        "RightHand" => Some(HumanoidBoneName::RightHand),
        "LeftUpLeg" => Some(HumanoidBoneName::LeftUpperLeg),
        "LeftLeg" => Some(HumanoidBoneName::LeftLowerLeg),
        "LeftFoot" => Some(HumanoidBoneName::LeftFoot),
        "LeftToeBase" => Some(HumanoidBoneName::LeftToes),
        "RightUpLeg" => Some(HumanoidBoneName::RightUpperLeg),
        "RightLeg" => Some(HumanoidBoneName::RightLowerLeg),
        "RightFoot" => Some(HumanoidBoneName::RightFoot),
        "RightToeBase" => Some(HumanoidBoneName::RightToes),
        "LeftHandThumb1" => Some(HumanoidBoneName::LeftThumbProximal),
        "LeftHandThumb2" => Some(HumanoidBoneName::LeftThumbIntermediate),
        "LeftHandThumb3" => Some(HumanoidBoneName::LeftThumbDistal),
        "LeftHandIndex1" => Some(HumanoidBoneName::LeftIndexProximal),
        "LeftHandIndex2" => Some(HumanoidBoneName::LeftIndexIntermediate),
        "LeftHandIndex3" => Some(HumanoidBoneName::LeftIndexDistal),
        "LeftHandMiddle1" => Some(HumanoidBoneName::LeftMiddleProximal),
        "LeftHandMiddle2" => Some(HumanoidBoneName::LeftMiddleIntermediate),
        "LeftHandMiddle3" => Some(HumanoidBoneName::LeftMiddleDistal),
        "LeftHandRing1" => Some(HumanoidBoneName::LeftRingProximal),
        "LeftHandRing2" => Some(HumanoidBoneName::LeftRingIntermediate),
        "LeftHandRing3" => Some(HumanoidBoneName::LeftRingDistal),
        "LeftHandPinky1" => Some(HumanoidBoneName::LeftLittleProximal),
        "LeftHandPinky2" => Some(HumanoidBoneName::LeftLittleIntermediate),
        "LeftHandPinky3" => Some(HumanoidBoneName::LeftLittleDistal),
        "RightHandThumb1" => Some(HumanoidBoneName::RightThumbProximal),
        "RightHandThumb2" => Some(HumanoidBoneName::RightThumbIntermediate),
        "RightHandThumb3" => Some(HumanoidBoneName::RightThumbDistal),
        "RightHandIndex1" => Some(HumanoidBoneName::RightIndexProximal),
        "RightHandIndex2" => Some(HumanoidBoneName::RightIndexIntermediate),
        "RightHandIndex3" => Some(HumanoidBoneName::RightIndexDistal),
        "RightHandMiddle1" => Some(HumanoidBoneName::RightMiddleProximal),
        "RightHandMiddle2" => Some(HumanoidBoneName::RightMiddleIntermediate),
        "RightHandMiddle3" => Some(HumanoidBoneName::RightMiddleDistal),
        "RightHandRing1" => Some(HumanoidBoneName::RightRingProximal),
        "RightHandRing2" => Some(HumanoidBoneName::RightRingIntermediate),
        "RightHandRing3" => Some(HumanoidBoneName::RightRingDistal),
        "RightHandPinky1" => Some(HumanoidBoneName::RightLittleProximal),
        "RightHandPinky2" => Some(HumanoidBoneName::RightLittleIntermediate),
        "RightHandPinky3" => Some(HumanoidBoneName::RightLittleDistal),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixamo_mapping_covers_major_bones() {
        assert_eq!(mixamo_to_vrm("mixamorig:Hips"), Some(HumanoidBoneName::Hips));
        assert_eq!(mixamo_to_vrm("mixamorig:Spine1"), Some(HumanoidBoneName::Chest));
        assert_eq!(mixamo_to_vrm("mixamorig:LeftArm"), Some(HumanoidBoneName::LeftUpperArm));
        assert_eq!(mixamo_to_vrm("mixamorig:RightForeArm"), Some(HumanoidBoneName::RightLowerArm));
        assert_eq!(mixamo_to_vrm("mixamorig:LeftUpLeg"), Some(HumanoidBoneName::LeftUpperLeg));
        assert_eq!(mixamo_to_vrm("mixamorig:HeadTop_End"), None);
        assert_eq!(mixamo_to_vrm("mixamorig:LeftHandThumb4"), None);
    }

    #[test]
    fn mixamo_mapping_finger_bones() {
        assert_eq!(mixamo_to_vrm("mixamorig:LeftHandThumb1"), Some(HumanoidBoneName::LeftThumbProximal));
        assert_eq!(mixamo_to_vrm("mixamorig:RightHandPinky3"), Some(HumanoidBoneName::RightLittleDistal));
    }

    #[test]
    fn compute_world_rotations_identity_chain() {
        let local = vec![Quat::IDENTITY, Quat::IDENTITY, Quat::IDENTITY];
        let parents = vec![None, Some(0), Some(1)];
        let world = compute_world_rotations(&local, &parents);
        for w in &world {
            assert!(w.dot(Quat::IDENTITY).abs() > 0.999);
        }
    }

    #[test]
    fn compute_world_rotations_accumulates() {
        let rot90y = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let local = vec![rot90y, rot90y];
        let parents = vec![None, Some(0)];
        let world = compute_world_rotations(&local, &parents);
        // Child should have 180° Y rotation
        let expected = Quat::from_rotation_y(std::f32::consts::PI);
        assert!(world[1].dot(expected).abs() > 0.99);
    }
}
