use crate::types::*;
use crate::utils::remap;
use glam::{Vec2, Vec3};

/// Solve pose rig from 33 pose landmarks (3D + 2D).
pub fn solve(landmarks_3d: &[Vec3], landmarks_2d: &[Vec2], video: &VideoInfo) -> RiggedPose {
    if landmarks_3d.len() < 33 || landmarks_2d.len() < 33 {
        return default_pose();
    }

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

    let right_upper_arm =
        calc_limb_rotation(landmarks_3d[12], landmarks_3d[14], landmarks_3d[16]);
    let right_lower_arm =
        calc_limb_rotation(landmarks_3d[14], landmarks_3d[16], landmarks_3d[18]);
    let left_upper_arm =
        calc_limb_rotation(landmarks_3d[11], landmarks_3d[13], landmarks_3d[15]);
    let left_lower_arm =
        calc_limb_rotation(landmarks_3d[13], landmarks_3d[15], landmarks_3d[17]);

    RiggedPose {
        hips,
        spine,
        chest: EulerAngles::new(spine.x * 0.5, spine.y * 0.5, spine.z * 0.5),
        right_upper_arm,
        right_lower_arm,
        left_upper_arm,
        left_lower_arm,
        right_upper_leg: calc_limb_rotation(
            landmarks_3d[24],
            landmarks_3d[26],
            landmarks_3d[28],
        ),
        right_lower_leg: calc_limb_rotation(
            landmarks_3d[26],
            landmarks_3d[28],
            landmarks_3d[30],
        ),
        left_upper_leg: calc_limb_rotation(
            landmarks_3d[23],
            landmarks_3d[25],
            landmarks_3d[27],
        ),
        left_lower_leg: calc_limb_rotation(
            landmarks_3d[25],
            landmarks_3d[27],
            landmarks_3d[29],
        ),
        left_hand: EulerAngles::default(),
        right_hand: EulerAngles::default(),
    }
}

fn default_pose() -> RiggedPose {
    RiggedPose {
        hips: HipTransform {
            rotation: EulerAngles::default(),
            position: Vec3::ZERO,
        },
        spine: EulerAngles::default(),
        chest: EulerAngles::default(),
        right_upper_arm: EulerAngles::default(),
        right_lower_arm: EulerAngles::default(),
        left_upper_arm: EulerAngles::default(),
        left_lower_arm: EulerAngles::default(),
        right_upper_leg: EulerAngles::default(),
        right_lower_leg: EulerAngles::default(),
        left_upper_leg: EulerAngles::default(),
        left_lower_leg: EulerAngles::default(),
        left_hand: EulerAngles::default(),
        right_hand: EulerAngles::default(),
    }
}

fn calc_hip_transform(lm3d: &[Vec3], lm2d: &[Vec2], video: &VideoInfo) -> HipTransform {
    // Hip position: midpoint of left hip (23) and right hip (24)
    let hip_center_3d = (lm3d[23] + lm3d[24]) * 0.5;
    let hip_center_2d = (lm2d[23] + lm2d[24]) * 0.5;

    // Normalize position to screen space
    let position = Vec3::new(
        remap(
            hip_center_2d.x,
            0.0,
            video.width as f32,
            -1.0,
            1.0,
        ),
        remap(
            hip_center_2d.y,
            0.0,
            video.height as f32,
            1.0,
            -1.0,
        ),
        hip_center_3d.z,
    );

    // Hip rotation from shoulder and hip vectors
    let left_shoulder = lm3d[11];
    let right_shoulder = lm3d[12];
    let left_hip = lm3d[23];
    let right_hip = lm3d[24];

    let shoulder_vec = (right_shoulder - left_shoulder).normalize();
    let hip_vec = (right_hip - left_hip).normalize();

    let yaw = shoulder_vec.z.atan2(shoulder_vec.x);
    let pitch = ((left_shoulder + right_shoulder) * 0.5 - (left_hip + right_hip) * 0.5)
        .normalize()
        .z
        .atan2(1.0);
    let roll = (hip_vec.y).atan2(hip_vec.x);

    HipTransform {
        rotation: EulerAngles {
            x: pitch,
            y: yaw,
            z: roll,
        },
        position,
    }
}

fn calc_spine_rotation(lm3d: &[Vec3]) -> EulerAngles {
    // Spine rotation from shoulder midpoint to hip midpoint
    let shoulder_mid = (lm3d[11] + lm3d[12]) * 0.5;
    let hip_mid = (lm3d[23] + lm3d[24]) * 0.5;
    let spine_dir = (shoulder_mid - hip_mid).normalize();

    EulerAngles {
        x: spine_dir.z.atan2(spine_dir.y),
        y: spine_dir.x.atan2(spine_dir.y),
        z: 0.0,
    }
}

fn calc_limb_rotation(a: Vec3, b: Vec3, c: Vec3) -> EulerAngles {
    let ab = (b - a).normalize();
    let bc = (c - b).normalize();
    EulerAngles {
        x: ab.y.atan2(ab.z),
        y: ab.x.atan2(ab.z),
        z: bc.x.atan2(bc.y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dummy_pose() -> (Vec<Vec3>, Vec<Vec2>) {
        let lm3d: Vec<Vec3> = (0..33)
            .map(|i| Vec3::new(i as f32 * 0.1, i as f32 * 0.05, 0.0))
            .collect();
        let lm2d: Vec<Vec2> = (0..33)
            .map(|i| Vec2::new(i as f32 * 20.0, i as f32 * 15.0))
            .collect();
        (lm3d, lm2d)
    }

    #[test]
    fn solve_with_valid_landmarks() {
        let (lm3d, lm2d) = make_dummy_pose();
        let video = VideoInfo {
            width: 640,
            height: 480,
        };
        let result = solve(&lm3d, &lm2d, &video);
        assert!(!result.spine.x.is_nan());
        assert!(!result.hips.position.x.is_nan());
    }

    #[test]
    fn solve_with_insufficient_landmarks_returns_default() {
        let lm3d = vec![Vec3::ZERO; 10];
        let lm2d = vec![Vec2::ZERO; 10];
        let video = VideoInfo {
            width: 640,
            height: 480,
        };
        let result = solve(&lm3d, &lm2d, &video);
        assert_eq!(result.spine.x, 0.0);
    }

    #[test]
    fn calc_limb_rotation_basic() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        let c = Vec3::new(0.0, 2.0, 0.0);
        let euler = calc_limb_rotation(a, b, c);
        assert!(!euler.x.is_nan());
    }
}
