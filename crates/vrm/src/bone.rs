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
        Ok(Self { bones })
    }

    pub fn get(&self, name: HumanoidBoneName) -> Option<&Bone> {
        self.bones.get(&name)
    }

    pub fn set_rotation(&mut self, name: HumanoidBoneName, rotation: Quat) {
        if let Some(bone) = self.bones.get_mut(&name) {
            bone.local_rotation = rotation;
        }
    }

    /// Forward Kinematicsで全ボーンのワールド行列を計算
    pub fn compute_joint_matrices(
        &self,
        node_transforms: &[crate::model::NodeTransform],
    ) -> Vec<Mat4> {
        let mut world_matrices = vec![Mat4::IDENTITY; node_transforms.len()];

        // Build bone node index lookup
        let bone_rotations: HashMap<usize, Quat> = self
            .bones
            .values()
            .map(|b| (b.node_index, b.local_rotation))
            .collect();

        // Process nodes in order (parent before children)
        for (i, nt) in node_transforms.iter().enumerate() {
            let rotation = bone_rotations.get(&i).copied().unwrap_or(nt.rotation);
            let local = Mat4::from_scale_rotation_translation(nt.scale, rotation, nt.translation);
            // Find parent by checking which node has this index as a child
            let parent_matrix = node_transforms
                .iter()
                .enumerate()
                .find(|(_, pnt)| pnt.children.contains(&i))
                .map(|(pi, _)| world_matrices[pi])
                .unwrap_or(Mat4::IDENTITY);
            world_matrices[i] = parent_matrix * local;
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
    fn missing_human_bones_key_returns_error() {
        let json: serde_json::Value = serde_json::from_str(r#"{ "humanoid": {} }"#).unwrap();
        let result = HumanoidBones::from_vrm_json(&json, &[]);
        assert!(result.is_err());
    }
}
