use std::collections::HashMap;

use glam::{Mat4, Quat, Vec3};

use crate::error::VrmError;

/// VRM 0.x Humanoid Bone Names (55種)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HumanoidBoneName {
    // Spine (6)
    Hips,
    Spine,
    Chest,
    UpperChest,
    Neck,
    Head,
    // Left Arm (4)
    LeftShoulder,
    LeftUpperArm,
    LeftLowerArm,
    LeftHand,
    // Right Arm (4)
    RightShoulder,
    RightUpperArm,
    RightLowerArm,
    RightHand,
    // Left Leg (4)
    LeftUpperLeg,
    LeftLowerLeg,
    LeftFoot,
    LeftToes,
    // Right Leg (4)
    RightUpperLeg,
    RightLowerLeg,
    RightFoot,
    RightToes,
    // Left Fingers (15)
    LeftThumbProximal,
    LeftThumbIntermediate,
    LeftThumbDistal,
    LeftIndexProximal,
    LeftIndexIntermediate,
    LeftIndexDistal,
    LeftMiddleProximal,
    LeftMiddleIntermediate,
    LeftMiddleDistal,
    LeftRingProximal,
    LeftRingIntermediate,
    LeftRingDistal,
    LeftLittleProximal,
    LeftLittleIntermediate,
    LeftLittleDistal,
    // Right Fingers (15)
    RightThumbProximal,
    RightThumbIntermediate,
    RightThumbDistal,
    RightIndexProximal,
    RightIndexIntermediate,
    RightIndexDistal,
    RightMiddleProximal,
    RightMiddleIntermediate,
    RightMiddleDistal,
    RightRingProximal,
    RightRingIntermediate,
    RightRingDistal,
    RightLittleProximal,
    RightLittleIntermediate,
    RightLittleDistal,
    // Eyes & Jaw (3)
    LeftEye,
    RightEye,
    Jaw,
}

impl HumanoidBoneName {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "hips" => Some(Self::Hips),
            "spine" => Some(Self::Spine),
            "chest" => Some(Self::Chest),
            "upperChest" => Some(Self::UpperChest),
            "neck" => Some(Self::Neck),
            "head" => Some(Self::Head),
            "leftShoulder" => Some(Self::LeftShoulder),
            "leftUpperArm" => Some(Self::LeftUpperArm),
            "leftLowerArm" => Some(Self::LeftLowerArm),
            "leftHand" => Some(Self::LeftHand),
            "rightShoulder" => Some(Self::RightShoulder),
            "rightUpperArm" => Some(Self::RightUpperArm),
            "rightLowerArm" => Some(Self::RightLowerArm),
            "rightHand" => Some(Self::RightHand),
            "leftUpperLeg" => Some(Self::LeftUpperLeg),
            "leftLowerLeg" => Some(Self::LeftLowerLeg),
            "leftFoot" => Some(Self::LeftFoot),
            "leftToes" => Some(Self::LeftToes),
            "rightUpperLeg" => Some(Self::RightUpperLeg),
            "rightLowerLeg" => Some(Self::RightLowerLeg),
            "rightFoot" => Some(Self::RightFoot),
            "rightToes" => Some(Self::RightToes),
            "leftThumbProximal" => Some(Self::LeftThumbProximal),
            "leftThumbIntermediate" => Some(Self::LeftThumbIntermediate),
            "leftThumbDistal" => Some(Self::LeftThumbDistal),
            "leftIndexProximal" => Some(Self::LeftIndexProximal),
            "leftIndexIntermediate" => Some(Self::LeftIndexIntermediate),
            "leftIndexDistal" => Some(Self::LeftIndexDistal),
            "leftMiddleProximal" => Some(Self::LeftMiddleProximal),
            "leftMiddleIntermediate" => Some(Self::LeftMiddleIntermediate),
            "leftMiddleDistal" => Some(Self::LeftMiddleDistal),
            "leftRingProximal" => Some(Self::LeftRingProximal),
            "leftRingIntermediate" => Some(Self::LeftRingIntermediate),
            "leftRingDistal" => Some(Self::LeftRingDistal),
            "leftLittleProximal" => Some(Self::LeftLittleProximal),
            "leftLittleIntermediate" => Some(Self::LeftLittleIntermediate),
            "leftLittleDistal" => Some(Self::LeftLittleDistal),
            "rightThumbProximal" => Some(Self::RightThumbProximal),
            "rightThumbIntermediate" => Some(Self::RightThumbIntermediate),
            "rightThumbDistal" => Some(Self::RightThumbDistal),
            "rightIndexProximal" => Some(Self::RightIndexProximal),
            "rightIndexIntermediate" => Some(Self::RightIndexIntermediate),
            "rightIndexDistal" => Some(Self::RightIndexDistal),
            "rightMiddleProximal" => Some(Self::RightMiddleProximal),
            "rightMiddleIntermediate" => Some(Self::RightMiddleIntermediate),
            "rightMiddleDistal" => Some(Self::RightMiddleDistal),
            "rightRingProximal" => Some(Self::RightRingProximal),
            "rightRingIntermediate" => Some(Self::RightRingIntermediate),
            "rightRingDistal" => Some(Self::RightRingDistal),
            "rightLittleProximal" => Some(Self::RightLittleProximal),
            "rightLittleIntermediate" => Some(Self::RightLittleIntermediate),
            "rightLittleDistal" => Some(Self::RightLittleDistal),
            "leftEye" => Some(Self::LeftEye),
            "rightEye" => Some(Self::RightEye),
            "jaw" => Some(Self::Jaw),
            _ => None,
        }
    }
}

/// 個々のボーン情報
pub struct Bone {
    pub node_index: usize,
    pub local_rotation: Quat,
    pub local_position: Vec3,
    pub inverse_bind_matrix: Mat4,
    pub children: Vec<usize>,
}

/// VRMヒューマノイドボーン集合
pub struct HumanoidBones {
    bones: HashMap<HumanoidBoneName, Bone>,
    prev_rotations: HashMap<HumanoidBoneName, Quat>,
    prev_positions: HashMap<HumanoidBoneName, Vec3>,
}

impl HumanoidBones {
    /// VRM拡張JSONからボーンマッピングを構築
    pub fn from_vrm_json(
        vrm_ext: &serde_json::Value,
        node_transforms: &[crate::model::NodeTransform],
    ) -> Result<Self, VrmError> {
        let human_bones = vrm_ext
            .get("humanoid")
            .and_then(|h| h.get("humanBones"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| VrmError::MissingExtension("humanoid.humanBones".into()))?;

        let mut bones = HashMap::new();
        for entry in human_bones {
            let bone_name = entry
                .get("bone")
                .and_then(|b| b.as_str())
                .ok_or_else(|| VrmError::MissingData("bone name".into()))?;
            let node_idx = entry
                .get("node")
                .and_then(|n| n.as_u64())
                .ok_or_else(|| VrmError::MissingData("bone node index".into()))?
                as usize;

            if let Some(name) = HumanoidBoneName::parse(bone_name) {
                let (local_position, local_rotation, children) =
                    if let Some(nt) = node_transforms.get(node_idx) {
                        (nt.translation, nt.rotation, nt.children.clone())
                    } else {
                        (Vec3::ZERO, Quat::IDENTITY, vec![])
                    };

                bones.insert(
                    name,
                    Bone {
                        node_index: node_idx,
                        local_rotation,
                        local_position,
                        inverse_bind_matrix: Mat4::IDENTITY,
                        children,
                    },
                );
            }
        }
        // Initialize prev_rotations and prev_positions from bind pose values.
        // In Three.js, Part.quaternion starts at the glTF bind pose, so
        // Part.quaternion.slerp(target, lerpAmount) interpolates FROM bind pose.
        // We must match this by initializing prev values from bind pose.
        let mut prev_rotations = HashMap::new();
        let mut prev_positions = HashMap::new();
        for (&name, bone) in &bones {
            prev_rotations.insert(name, bone.local_rotation);
            prev_positions.insert(name, bone.local_position);
        }

        Ok(Self {
            bones,
            prev_rotations,
            prev_positions,
        })
    }

    pub fn get(&self, name: HumanoidBoneName) -> Option<&Bone> {
        self.bones.get(&name)
    }

    pub fn set_rotation(&mut self, name: HumanoidBoneName, rotation: Quat) {
        if let Some(bone) = self.bones.get_mut(&name) {
            bone.local_rotation = rotation;
        }
    }

    pub fn set_position(&mut self, name: HumanoidBoneName, position: Vec3) {
        if let Some(bone) = self.bones.get_mut(&name) {
            bone.local_position = position;
        }
    }

    /// Set position with dampener and lerp interpolation (matching KalidoKit rigPosition).
    ///
    /// 1. Apply dampener: scale `target` by `dampener`
    /// 2. Interpolate from previous position toward dampened target by `lerp_amount`
    pub fn set_position_interpolated(
        &mut self,
        name: HumanoidBoneName,
        target: Vec3,
        dampener: f32,
        lerp_amount: f32,
    ) {
        let dampened = target * dampener;
        let prev = self
            .prev_positions
            .get(&name)
            .copied()
            .unwrap_or(Vec3::ZERO);
        let interpolated = prev.lerp(dampened, lerp_amount);
        self.prev_positions.insert(name, interpolated);
        if let Some(bone) = self.bones.get_mut(&name) {
            bone.local_position = interpolated;
        }
    }

    /// Set rotation with dampener and slerp interpolation (matching KalidoKit rigRotation).
    ///
    /// The caller must apply dampener to the Euler angles BEFORE converting to
    /// quaternion, matching the testbed: `Euler(x*d, y*d, z*d) → Quat → slerp`.
    /// The `target` quaternion passed here is already dampened.
    pub fn set_rotation_interpolated(
        &mut self,
        name: HumanoidBoneName,
        target: Quat,
        lerp_amount: f32,
    ) {
        let prev = self
            .prev_rotations
            .get(&name)
            .copied()
            .unwrap_or(Quat::IDENTITY);
        let interpolated = prev.slerp(target, lerp_amount);
        self.prev_rotations.insert(name, interpolated);
        if let Some(bone) = self.bones.get_mut(&name) {
            bone.local_rotation = interpolated;
        }
    }

    /// Forward Kinematicsで全ボーンのワールド行列を計算
    ///
    /// glTFではノードインデックスの親子順序が保証されないため、
    /// BFSでルートから子へ順に処理する（トポロジカル順序）。
    pub fn compute_joint_matrices(
        &self,
        node_transforms: &[crate::model::NodeTransform],
    ) -> Vec<Mat4> {
        let n = node_transforms.len();
        let mut world_matrices = vec![Mat4::IDENTITY; n];
        let mut computed = vec![false; n];

        // Build bone node index lookups
        let bone_rotations: HashMap<usize, Quat> = self
            .bones
            .values()
            .map(|b| (b.node_index, b.local_rotation))
            .collect();
        let bone_positions: HashMap<usize, Vec3> = self
            .bones
            .values()
            .map(|b| (b.node_index, b.local_position))
            .collect();

        // Build parent lookup: parent_of[child] = parent_index
        let mut parent_of = vec![None::<usize>; n];
        for (i, nt) in node_transforms.iter().enumerate() {
            for &child in &nt.children {
                if child < n {
                    parent_of[child] = Some(i);
                }
            }
        }

        // Find root nodes (nodes with no parent) and BFS from them
        let mut queue = std::collections::VecDeque::new();
        for i in 0..n {
            if parent_of[i].is_none() {
                queue.push_back(i);
            }
        }

        while let Some(i) = queue.pop_front() {
            if computed[i] {
                continue;
            }
            let nt = &node_transforms[i];
            let rotation = bone_rotations.get(&i).copied().unwrap_or(nt.rotation);
            let translation = bone_positions.get(&i).copied().unwrap_or(nt.translation);
            let local = Mat4::from_scale_rotation_translation(nt.scale, rotation, translation);
            let parent_matrix = parent_of[i]
                .map(|pi| world_matrices[pi])
                .unwrap_or(Mat4::IDENTITY);
            world_matrices[i] = parent_matrix * local;
            computed[i] = true;

            // Enqueue children
            for &child in &nt.children {
                if child < n {
                    queue.push_back(child);
                }
            }
        }

        world_matrices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_hips() {
        assert_eq!(
            HumanoidBoneName::parse("hips"),
            Some(HumanoidBoneName::Hips)
        );
    }

    #[test]
    fn from_str_left_upper_arm() {
        assert_eq!(
            HumanoidBoneName::parse("leftUpperArm"),
            Some(HumanoidBoneName::LeftUpperArm)
        );
    }

    #[test]
    fn from_str_invalid() {
        assert_eq!(HumanoidBoneName::parse("invalid_bone"), None);
    }

    #[test]
    fn from_str_all_55_bones() {
        let names = [
            "hips",
            "spine",
            "chest",
            "upperChest",
            "neck",
            "head",
            "leftShoulder",
            "leftUpperArm",
            "leftLowerArm",
            "leftHand",
            "rightShoulder",
            "rightUpperArm",
            "rightLowerArm",
            "rightHand",
            "leftUpperLeg",
            "leftLowerLeg",
            "leftFoot",
            "leftToes",
            "rightUpperLeg",
            "rightLowerLeg",
            "rightFoot",
            "rightToes",
            "leftThumbProximal",
            "leftThumbIntermediate",
            "leftThumbDistal",
            "leftIndexProximal",
            "leftIndexIntermediate",
            "leftIndexDistal",
            "leftMiddleProximal",
            "leftMiddleIntermediate",
            "leftMiddleDistal",
            "leftRingProximal",
            "leftRingIntermediate",
            "leftRingDistal",
            "leftLittleProximal",
            "leftLittleIntermediate",
            "leftLittleDistal",
            "rightThumbProximal",
            "rightThumbIntermediate",
            "rightThumbDistal",
            "rightIndexProximal",
            "rightIndexIntermediate",
            "rightIndexDistal",
            "rightMiddleProximal",
            "rightMiddleIntermediate",
            "rightMiddleDistal",
            "rightRingProximal",
            "rightRingIntermediate",
            "rightRingDistal",
            "rightLittleProximal",
            "rightLittleIntermediate",
            "rightLittleDistal",
            "leftEye",
            "rightEye",
            "jaw",
        ];
        for name in &names {
            assert!(
                HumanoidBoneName::parse(name).is_some(),
                "Failed for bone: {}",
                name
            );
        }
        assert_eq!(names.len(), 55);
    }

    #[test]
    fn from_vrm_json_parses_bones() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "humanoid": {
                    "humanBones": [
                        { "bone": "hips", "node": 0 },
                        { "bone": "spine", "node": 1 },
                        { "bone": "head", "node": 2 }
                    ]
                }
            }"#,
        )
        .unwrap();

        let node_transforms = vec![
            crate::model::NodeTransform {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                children: vec![1],
            },
            crate::model::NodeTransform {
                translation: Vec3::new(0.0, 0.5, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                children: vec![2],
            },
            crate::model::NodeTransform {
                translation: Vec3::new(0.0, 0.3, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                children: vec![],
            },
        ];

        let bones = HumanoidBones::from_vrm_json(&json, &node_transforms).unwrap();
        assert!(bones.get(HumanoidBoneName::Hips).is_some());
        assert!(bones.get(HumanoidBoneName::Spine).is_some());
        assert!(bones.get(HumanoidBoneName::Head).is_some());
        assert!(bones.get(HumanoidBoneName::LeftHand).is_none());
    }

    #[test]
    fn slerp_interpolation_produces_intermediate_value() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "humanoid": {
                    "humanBones": [
                        { "bone": "hips", "node": 0 }
                    ]
                }
            }"#,
        )
        .unwrap();

        let node_transforms = vec![crate::model::NodeTransform {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            children: vec![],
        }];

        let mut bones = HumanoidBones::from_vrm_json(&json, &node_transforms).unwrap();

        // Target: 90 degrees around Y axis
        let target = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);

        // lerp_amount=0.3 means 30% toward target (dampener now applied by caller before quat conversion)
        bones.set_rotation_interpolated(HumanoidBoneName::Hips, target, 0.3);

        let result = bones.get(HumanoidBoneName::Hips).unwrap().local_rotation;

        // Result should be between IDENTITY and target
        let angle = result.angle_between(Quat::IDENTITY);
        assert!(angle > 0.01, "Result should differ from IDENTITY");
        assert!(
            angle < std::f32::consts::FRAC_PI_2 - 0.01,
            "Result should be less than the full 90-degree target"
        );
    }

    #[test]
    fn missing_human_bones_key_returns_error() {
        let json: serde_json::Value = serde_json::from_str(r#"{ "humanoid": {} }"#).unwrap();
        let result = HumanoidBones::from_vrm_json(&json, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn set_position_applied_in_joint_matrices() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "humanoid": {
                    "humanBones": [
                        { "bone": "hips", "node": 0 }
                    ]
                }
            }"#,
        )
        .unwrap();

        let node_transforms = vec![crate::model::NodeTransform {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            children: vec![],
        }];

        let mut bones = HumanoidBones::from_vrm_json(&json, &node_transforms).unwrap();

        let target_pos = Vec3::new(1.0, 2.0, 3.0);
        bones.set_position(HumanoidBoneName::Hips, target_pos);

        let matrices = bones.compute_joint_matrices(&node_transforms);

        // The Hips node (index 0) has no parent, so world matrix = local matrix.
        // With IDENTITY rotation, ONE scale, the translation column should be target_pos.
        let world = matrices[0];
        let translation = world.col(3).truncate();
        assert!(
            (translation - target_pos).length() < 1e-5,
            "Expected translation {:?}, got {:?}",
            target_pos,
            translation
        );
    }
}
