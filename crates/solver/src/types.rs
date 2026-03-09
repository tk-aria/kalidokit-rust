use glam::{EulerRot, Quat, Vec2, Vec3};

/// Video metadata for coordinate normalization
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
}

/// Left or Right side
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Face solver result
#[derive(Debug, Clone)]
pub struct RiggedFace {
    pub head: EulerAngles,
    pub eye: EyeValues,
    pub pupil: Vec2,
    pub mouth: MouthShape,
    pub brow: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EulerAngles {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl EulerAngles {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn to_quat(&self) -> Quat {
        Quat::from_euler(EulerRot::XYZ, self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EyeValues {
    pub l: f32,
    pub r: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MouthShape {
    pub a: f32,
    pub i: f32,
    pub u: f32,
    pub e: f32,
    pub o: f32,
}

/// Pose solver result
#[derive(Debug, Clone)]
pub struct RiggedPose {
    pub hips: HipTransform,
    pub spine: EulerAngles,
    pub chest: EulerAngles,
    pub right_upper_arm: EulerAngles,
    pub right_lower_arm: EulerAngles,
    pub left_upper_arm: EulerAngles,
    pub left_lower_arm: EulerAngles,
    pub right_upper_leg: EulerAngles,
    pub right_lower_leg: EulerAngles,
    pub left_upper_leg: EulerAngles,
    pub left_lower_leg: EulerAngles,
    pub left_hand: EulerAngles,
    pub right_hand: EulerAngles,
}

#[derive(Debug, Clone)]
pub struct HipTransform {
    pub rotation: EulerAngles,
    pub position: Vec3,
}

/// Hand solver result (per hand)
#[derive(Debug, Clone)]
pub struct RiggedHand {
    pub wrist: EulerAngles,
    pub thumb_proximal: EulerAngles,
    pub thumb_intermediate: EulerAngles,
    pub thumb_distal: EulerAngles,
    pub index_proximal: EulerAngles,
    pub index_intermediate: EulerAngles,
    pub index_distal: EulerAngles,
    pub middle_proximal: EulerAngles,
    pub middle_intermediate: EulerAngles,
    pub middle_distal: EulerAngles,
    pub ring_proximal: EulerAngles,
    pub ring_intermediate: EulerAngles,
    pub ring_distal: EulerAngles,
    pub little_proximal: EulerAngles,
    pub little_intermediate: EulerAngles,
    pub little_distal: EulerAngles,
}
