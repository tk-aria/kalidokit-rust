use std::f32::consts::PI;

use crate::types::*;
use crate::utils::{
    angle_between_3d_coords, clamp, find_rotation, lerp_vec3, normalize_angle, remap01,
    roll_pitch_yaw,
};
use glam::{Vec2, Vec3};

/// Solve pose rig from 33 pose landmarks (3D + 2D).
pub fn solve(landmarks_3d: &[Vec3], landmarks_2d: &[Vec2], _video: &VideoInfo) -> RiggedPose {
    if landmarks_3d.len() < 33 || landmarks_2d.len() < 33 {
        return default_pose();
    }

    let lm = landmarks_3d;
    let lm2d = landmarks_2d;

    // --- Hips ---
    let (hips, spine) = calc_hips(lm, lm2d);

    // --- Arms ---
    let (right_upper_arm, right_lower_arm, right_hand) = calc_arms(lm, Side::Right);
    let (left_upper_arm, left_lower_arm, left_hand) = calc_arms(lm, Side::Left);

    // --- Legs (matching KalidoKit calcLegs + rigLeg) ---
    let (right_upper_leg, right_lower_leg) = calc_legs(lm, Side::Right);
    let (left_upper_leg, left_lower_leg) = calc_legs(lm, Side::Left);

    // Offscreen detection: reset to defaults when limbs are unreliable
    // KalidoKit: hand offscreen if lm3d[idx].y > 0.1 or visibility < 0.23 or lm2d[idx].y > 0.995
    let right_hand_offscreen = lm[15].y > 0.1 || lm2d[15].y > 0.995;
    let left_hand_offscreen = lm[16].y > 0.1 || lm2d[16].y > 0.995;
    let left_foot_offscreen = lm[23].y > 0.1 || hips.position.z > -0.4;
    let right_foot_offscreen = lm[24].y > 0.1 || hips.position.z > -0.4;

    // Resting defaults from KalidoKit
    let mut right_upper_arm = right_upper_arm;
    let mut right_lower_arm = right_lower_arm;
    let mut right_hand = right_hand;
    let mut left_upper_arm = left_upper_arm;
    let mut left_lower_arm = left_lower_arm;
    let mut left_hand = left_hand;
    let mut right_upper_leg = right_upper_leg;
    let mut right_lower_leg = right_lower_leg;
    let mut left_upper_leg = left_upper_leg;
    let mut left_lower_leg = left_lower_leg;

    if left_hand_offscreen {
        left_upper_arm = EulerAngles::new(0.0, 0.0, 1.25);
        left_lower_arm = EulerAngles::default();
        left_hand = EulerAngles::default();
    }
    if right_hand_offscreen {
        right_upper_arm = EulerAngles::new(0.0, 0.0, -1.25);
        right_lower_arm = EulerAngles::default();
        right_hand = EulerAngles::default();
    }
    if right_foot_offscreen {
        left_upper_leg = EulerAngles::default();
        left_lower_leg = EulerAngles::default();
    }
    if left_foot_offscreen {
        right_upper_leg = EulerAngles::default();
        right_lower_leg = EulerAngles::default();
    }

    RiggedPose {
        hips,
        spine,
        chest: EulerAngles::new(spine.x * 0.5, spine.y * 0.5, spine.z * 0.5),
        right_upper_arm,
        right_lower_arm,
        left_upper_arm,
        left_lower_arm,
        right_upper_leg,
        right_lower_leg,
        left_upper_leg,
        left_lower_leg,
        left_hand,
        right_hand,
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

/// Calculate arms matching TypeScript KalidoKit calcArms + rigArm.
fn calc_arms(lm: &[Vec3], side: Side) -> (EulerAngles, EulerAngles, EulerAngles) {
    // MediaPipe indices:
    // Right: shoulder=11->12? No. Let's be precise:
    //   Right shoulder=12, right elbow=14, right wrist=16, right pinky=18, right index=20
    //   Left shoulder=11, left elbow=13, left wrist=15, left pinky=17, left index=19
    //
    // TypeScript calcArms:
    //   UpperArm.r = findRotation(lm[11], lm[13])  -- but lm[11] is LEFT shoulder, lm[13] is LEFT elbow
    //   Wait, looking at the task description again:
    //   UpperArm.r = findRotation(lm[11], lm[13])
    //   UpperArm.l = findRotation(lm[12], lm[14])
    //
    //   This is the TypeScript convention where .r and .l seem swapped relative to MediaPipe naming.
    //   Actually in KalidoKit TS, it's body-relative: right arm uses landmarks on the left side of image.
    //   MediaPipe: 11=left_shoulder, 12=right_shoulder, 13=left_elbow, 14=right_elbow
    //
    //   So UpperArm.r uses lm[11](left_shoulder) and lm[13](left_elbow) -- this IS the right arm
    //   from the avatar's perspective (mirrored from camera).
    //
    //   Actually no -- KalidoKit uses the MediaPipe convention directly. Let me re-read:
    //   In KalidoKit TS: UpperArm.r = findRotation(lm[11], lm[13])
    //   lm[11] = left_shoulder, lm[13] = left_elbow
    //   This would be the LEFT arm landmarks, assigned to "right" -- this is the mirror mapping.

    let (shoulder_idx, elbow_idx, wrist_idx, pinky_idx, index_idx, other_shoulder_idx) = match side
    {
        // Right side of avatar = left side landmarks (mirrored)
        Side::Right => (11usize, 13, 15, 17, 19, 12),
        Side::Left => (12, 14, 16, 18, 20, 11),
    };

    let shoulder = lm[shoulder_idx];
    let elbow = lm[elbow_idx];
    let wrist = lm[wrist_idx];
    let pinky = lm[pinky_idx];
    let index_tip = lm[index_idx];
    let other_shoulder = lm[other_shoulder_idx];

    // findRotation returns normalized [-1, 1] values
    let mut upper_arm = find_rotation(shoulder, elbow, true);
    let mut lower_arm = find_rotation(elbow, wrist, true);

    // Override y with angleBetween3DCoords
    upper_arm.y = angle_between_3d_coords(other_shoulder, shoulder, elbow);
    lower_arm.y = angle_between_3d_coords(shoulder, elbow, wrist);

    // Clamp lower arm z
    lower_arm.z = clamp(lower_arm.z, -2.14, 0.0);

    // Hand rotation: findRotation(wrist, lerp(pinky, index, 0.5))
    let hand_target = lerp_vec3(pinky, index_tip, 0.5);
    let hand = find_rotation(wrist, hand_target, true);

    // rigArm
    let invert = match side {
        Side::Right => 1.0,
        Side::Left => -1.0,
    };

    upper_arm.z *= -2.3 * invert;
    upper_arm.y *= PI * invert;
    // JS Math.max(LowerArm.x) with 1 arg returns the value unchanged
    upper_arm.y -= lower_arm.x;
    upper_arm.y -= -invert * lower_arm.z.max(0.0);
    upper_arm.x -= 0.3 * invert;

    lower_arm.z *= -2.14 * invert;
    lower_arm.y *= 2.14 * invert;
    lower_arm.x *= 2.14 * invert;

    upper_arm.x = clamp(upper_arm.x, -0.5, PI);
    lower_arm.x = clamp(lower_arm.x, -0.3, 0.3);

    let hand_euler = EulerAngles {
        x: 0.0,
        y: clamp(hand.z * 2.0, -0.6, 0.6),
        z: hand.z * -2.3 * invert,
    };

    (
        EulerAngles::new(upper_arm.x, upper_arm.y, upper_arm.z),
        EulerAngles::new(lower_arm.x, lower_arm.y, lower_arm.z),
        hand_euler,
    )
}

/// Apply hip/spine rotation fixups matching TypeScript KalidoKit calcHips.
fn apply_hip_spine_fixups(mut rotation: Vec3) -> Vec3 {
    if rotation.y > 0.5 {
        rotation.y -= 2.0;
    }
    rotation.y += 0.5;
    if rotation.z > 0.0 {
        rotation.z = 1.0 - rotation.z;
    }
    if rotation.z < 0.0 {
        rotation.z = -1.0 - rotation.z;
    }
    let turn_around = remap01(rotation.y.abs(), 0.2, 0.4);
    rotation.z *= 1.0 - turn_around;
    rotation.x = 0.0;
    rotation
}

/// Calculate hips and spine matching TypeScript KalidoKit calcHips.
fn calc_hips(lm3d: &[Vec3], lm2d: &[Vec2]) -> (HipTransform, EulerAngles) {
    let _hip_left_2d = lm2d[23];
    let hip_right_2d = lm2d[24];
    let _shoulder_left_2d = lm2d[11];
    let shoulder_right_2d = lm2d[12];

    // TypeScript uses lerp(..., 1) which gives the second point (quirk/bug in original)
    let hip_center_2d = hip_right_2d;
    let shoulder_center_2d = shoulder_right_2d;

    let spine_length = hip_center_2d.distance(shoulder_center_2d);

    let position = Vec3::new(
        clamp(hip_center_2d.x - 0.4, -1.0, 1.0),
        0.0,
        clamp(spine_length - 1.0, -2.0, 0.0),
    );

    // Hip rotation: rollPitchYaw 2-point mode on 3D hip landmarks
    let hip_rpy = roll_pitch_yaw(lm3d[23], lm3d[24], None);
    let mut hip_rotation = apply_hip_spine_fixups(hip_rpy);

    // Spine rotation: rollPitchYaw 2-point mode on 3D shoulder landmarks
    let spine_rpy = roll_pitch_yaw(lm3d[11], lm3d[12], None);
    let mut spine_rotation = apply_hip_spine_fixups(spine_rpy);

    // Scale to radians
    hip_rotation *= PI;
    spine_rotation *= PI;

    (
        HipTransform {
            rotation: EulerAngles::new(hip_rotation.x, hip_rotation.y, hip_rotation.z),
            position,
        },
        EulerAngles::new(spine_rotation.x, spine_rotation.y, spine_rotation.z),
    )
}

/// Calculate leg rotations matching KalidoKit calcLegs + rigLeg.
/// Uses spherical coordinates with axis remapping {x: "y", y: "z", z: "x"}.
fn calc_legs(lm: &[Vec3], side: Side) -> (EulerAngles, EulerAngles) {
    let invert = match side {
        Side::Right => 1.0_f32,
        Side::Left => -1.0,
    };

    // Landmark indices: Right hip=23, knee=25, ankle=27; Left hip=24, knee=26, ankle=28
    let (hip_idx, knee_idx, ankle_idx) = match side {
        Side::Right => (23usize, 25, 27),
        Side::Left => (24, 26, 28),
    };

    let hip = lm[hip_idx];
    let knee = lm[knee_idx];
    let ankle = lm[ankle_idx];

    // getSphericalCoords(hip, knee, {x:"y", y:"z", z:"x"})
    let upper_spherical = get_spherical_coords(hip, knee);
    // getRelativeSphericalCoords(hip, knee, ankle, {x:"y", y:"z", z:"x"})
    let lower_relative = get_relative_spherical_coords(hip, knee, ankle);

    // hipRotation = findRotation(lm[23], lm[24])
    let hip_rotation = find_rotation(lm[23], lm[24], true);

    // UpperLeg = { x: upperSpherical.theta, y: lowerRelative.phi, z: upperSpherical.phi - hipRotation.z }
    let upper_leg_raw = Vec3::new(
        upper_spherical.0,                  // theta
        lower_relative.1,                   // phi
        upper_spherical.1 - hip_rotation.z, // phi - hipRotation.z
    );

    // LowerLeg = { x: -abs(lowerRelative.theta), y: 0, z: 0 }
    let lower_leg_raw = Vec3::new(-lower_relative.0.abs(), 0.0, 0.0);

    // rigLeg: clamp and scale by PI
    let upper_leg = EulerAngles {
        x: clamp(upper_leg_raw.x, 0.0, 0.5) * PI,
        y: clamp(upper_leg_raw.y, -0.25, 0.25) * PI,
        z: clamp(upper_leg_raw.z, -0.5, 0.5) * PI + invert * 0.1,
    };
    let lower_leg = EulerAngles {
        x: lower_leg_raw.x * PI,
        y: lower_leg_raw.y * PI,
        z: lower_leg_raw.z * PI,
    };

    (upper_leg, lower_leg)
}

/// Get spherical coordinates with axis mapping {x:"y", y:"z", z:"x"}.
/// Returns (theta, phi) normalized to [-1, 1].
/// Matches KalidoKit Vector.getSphericalCoords.
fn get_spherical_coords(a: Vec3, b: Vec3) -> (f32, f32) {
    let v = (b - a).normalize_or_zero();
    // With axis map {x:"y", y:"z", z:"x"}: mapped.x = v.y, mapped.y = v.z, mapped.z = v.x
    let theta = v.z.atan2(v.y); // atan2(mapped.y, mapped.x)
    let len = v.length();
    let phi = if len > 1e-10 {
        (v.x / len).clamp(-1.0, 1.0).acos()
    } else {
        0.0
    }; // acos(mapped.z / length)
    (normalize_angle(-theta), normalize_angle(PI / 2.0 - phi))
}

/// Get relative spherical coordinates with axis mapping {x:"y", y:"z", z:"x"}.
/// Returns (theta, phi) normalized to [-1, 1].
/// Matches KalidoKit Vector.getRelativeSphericalCoords.
fn get_relative_spherical_coords(a: Vec3, b: Vec3, c: Vec3) -> (f32, f32) {
    let v1 = (b - a).normalize_or_zero();
    let v2 = (c - b).normalize_or_zero();
    // With axis map {x:"y", y:"z", z:"x"}
    let theta1 = v1.z.atan2(v1.y);
    let len1 = v1.length();
    let phi1 = if len1 > 1e-10 {
        (v1.x / len1).clamp(-1.0, 1.0).acos()
    } else {
        0.0
    };
    let theta2 = v2.z.atan2(v2.y);
    let len2 = v2.length();
    let phi2 = if len2 > 1e-10 {
        (v2.x / len2).clamp(-1.0, 1.0).acos()
    } else {
        0.0
    };
    (
        normalize_angle(theta1 - theta2),
        normalize_angle(phi1 - phi2),
    )
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
    fn t_pose_arm_rotation_finite() {
        let mut lm3d = vec![Vec3::ZERO; 33];
        lm3d[11] = Vec3::new(-1.0, 1.5, 0.0);
        lm3d[12] = Vec3::new(1.0, 1.5, 0.0);
        lm3d[13] = Vec3::new(-2.0, 1.5, 0.0);
        lm3d[14] = Vec3::new(2.0, 1.5, 0.0);
        lm3d[15] = Vec3::new(-3.0, 1.5, 0.0);
        lm3d[16] = Vec3::new(3.0, 1.5, 0.0);
        lm3d[17] = Vec3::new(-3.5, 1.5, 0.0);
        lm3d[18] = Vec3::new(3.5, 1.5, 0.0);
        lm3d[19] = Vec3::new(-3.5, 1.4, 0.0);
        lm3d[20] = Vec3::new(3.5, 1.4, 0.0);
        lm3d[23] = Vec3::new(-0.5, 0.0, 0.0);
        lm3d[24] = Vec3::new(0.5, 0.0, 0.0);
        lm3d[25] = Vec3::new(-0.5, -1.0, 0.0);
        lm3d[26] = Vec3::new(0.5, -1.0, 0.0);
        lm3d[27] = Vec3::new(-0.5, -2.0, 0.0);
        lm3d[28] = Vec3::new(0.5, -2.0, 0.0);
        lm3d[29] = Vec3::new(-0.5, -2.5, 0.0);
        lm3d[30] = Vec3::new(0.5, -2.5, 0.0);

        let lm2d: Vec<Vec2> = lm3d
            .iter()
            .map(|v| Vec2::new(v.x * 100.0 + 320.0, -v.y * 100.0 + 240.0))
            .collect();
        let video = VideoInfo {
            width: 640,
            height: 480,
        };
        let result = solve(&lm3d, &lm2d, &video);
        assert!(result.right_upper_arm.x.is_finite());
        assert!(result.left_upper_arm.x.is_finite());
        assert!(result.right_hand.y.is_finite());
        assert!(result.left_hand.z.is_finite());
    }

    #[test]
    fn hip_position_clamped() {
        let (lm3d, lm2d) = make_dummy_pose();
        let video = VideoInfo {
            width: 640,
            height: 480,
        };
        let result = solve(&lm3d, &lm2d, &video);
        assert!(result.hips.position.x >= -1.0 && result.hips.position.x <= 1.0);
        assert!(result.hips.position.z >= -2.0 && result.hips.position.z <= 0.0);
    }

    #[test]
    fn chest_is_half_spine() {
        let (lm3d, lm2d) = make_dummy_pose();
        let video = VideoInfo {
            width: 640,
            height: 480,
        };
        let result = solve(&lm3d, &lm2d, &video);
        assert!((result.chest.x - result.spine.x * 0.5).abs() < 1e-6);
        assert!((result.chest.y - result.spine.y * 0.5).abs() < 1e-6);
        assert!((result.chest.z - result.spine.z * 0.5).abs() < 1e-6);
    }
}
