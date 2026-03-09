use crate::types::*;
use glam::{Vec2, Vec3};

/// Solve pose rig from 33 pose landmarks (3D + 2D).
///
/// Computes bone rotations for hips, spine, chest, arms, and legs.
/// 3D landmarks are in meters relative to hip center.
/// 2D landmarks are normalized to video dimensions.
pub fn solve(landmarks_3d: &[Vec3], landmarks_2d: &[Vec2], video: &VideoInfo) -> RiggedPose {
    let hips = calc_hip_transform(landmarks_3d, landmarks_2d, video);
    let spine = calc_spine_rotation(landmarks_3d);

    // MediaPipe pose landmark indices:
    // 11=left_shoulder, 12=right_shoulder
    // 13=left_elbow, 14=right_elbow
    // 15=left_wrist, 16=right_wrist
    // 17=left_pinky, 18=right_pinky
    // 23=left_hip, 24=right_hip
    // 25=left_knee, 26=right_knee
    // 27=left_ankle, 28=right_ankle
    // 29=left_heel, 30=right_heel

    let right_upper_arm = calc_limb_rotation(landmarks_3d[12], landmarks_3d[14], landmarks_3d[16]);
    let right_lower_arm = calc_limb_rotation(landmarks_3d[14], landmarks_3d[16], landmarks_3d[18]);
    let left_upper_arm = calc_limb_rotation(landmarks_3d[11], landmarks_3d[13], landmarks_3d[15]);
    let left_lower_arm = calc_limb_rotation(landmarks_3d[13], landmarks_3d[15], landmarks_3d[17]);

    RiggedPose {
        hips,
        spine,
        chest: EulerAngles::new(spine.x, spine.y, spine.z),
        right_upper_arm,
        right_lower_arm,
        left_upper_arm,
        left_lower_arm,
        right_upper_leg: calc_limb_rotation(landmarks_3d[24], landmarks_3d[26], landmarks_3d[28]),
        right_lower_leg: calc_limb_rotation(landmarks_3d[26], landmarks_3d[28], landmarks_3d[30]),
        left_upper_leg: calc_limb_rotation(landmarks_3d[23], landmarks_3d[25], landmarks_3d[27]),
        left_lower_leg: calc_limb_rotation(landmarks_3d[25], landmarks_3d[27], landmarks_3d[29]),
        left_hand: EulerAngles::default(),
        right_hand: EulerAngles::default(),
    }
}

fn calc_hip_transform(lm3d: &[Vec3], lm2d: &[Vec2], video: &VideoInfo) -> HipTransform {
    let _ = (lm3d, lm2d, video);
    todo!("Port KalidoKit hip transform calculation")
}

fn calc_spine_rotation(lm3d: &[Vec3]) -> EulerAngles {
    let _ = lm3d;
    todo!("Port KalidoKit spine rotation calculation")
}

fn calc_limb_rotation(a: Vec3, b: Vec3, c: Vec3) -> EulerAngles {
    // Calculate euler angles from 3 joint positions (parent, current, child).
    let _ = (a, b, c);
    todo!("Port KalidoKit limb rotation calculation")
}
