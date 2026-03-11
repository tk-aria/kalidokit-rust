use std::f32::consts::PI;

use crate::types::*;
use crate::utils::{angle_between_3d_coords, clamp, roll_pitch_yaw};
use glam::Vec3;

/// Solve hand rig from 21 hand landmarks.
///
/// Computes wrist rotation and individual finger joint rotations
/// for all 5 fingers (3 joints each = 15 finger joints + wrist).
pub fn solve(landmarks: &[Vec3], side: Side) -> RiggedHand {
    if landmarks.len() < 21 {
        return default_hand();
    }

    let lm = landmarks;
    let invert: f32 = match side {
        Side::Right => 1.0,
        Side::Left => -1.0,
    };

    // --- Wrist rotation (TypeScript HandSolver) ---
    // palm = [lm[0], lm[side==Right ? 17 : 5], lm[side==Right ? 5 : 17]]
    let (palm_b, palm_c) = match side {
        Side::Right => (lm[17], lm[5]),
        Side::Left => (lm[5], lm[17]),
    };
    let mut hand_rotation = roll_pitch_yaw(lm[0], palm_b, Some(palm_c));
    // TypeScript: handRotation.y = handRotation.z; handRotation.y -= 0.4
    hand_rotation.y = hand_rotation.z;
    hand_rotation.y -= 0.4;

    // rigFingers wrist fixups
    let wrist = EulerAngles {
        x: clamp(hand_rotation.x * 2.0 * invert, -0.3, 0.3),
        y: clamp(
            hand_rotation.y * 2.3,
            if side == Side::Right { -1.2 } else { -0.6 },
            if side == Side::Right { 0.6 } else { 1.6 },
        ),
        z: hand_rotation.z * -2.3 * invert,
    };

    // --- Finger joints ---
    // Landmark indices per finger (includes wrist/base at index 0):
    // Thumb:  0, 1, 2, 3, 4
    // Index:  0, 5, 6, 7, 8
    // Middle: 0, 9, 10, 11, 12
    // Ring:   0, 13, 14, 15, 16
    // Little: 0, 17, 18, 19, 20

    let thumb = calc_thumb(lm, side, invert);
    let index = calc_non_thumb_finger(lm, &[0, 5, 6, 7, 8], side, invert);
    let middle = calc_non_thumb_finger(lm, &[0, 9, 10, 11, 12], side, invert);
    let ring = calc_non_thumb_finger(lm, &[0, 13, 14, 15, 16], side, invert);
    let little = calc_non_thumb_finger(lm, &[0, 17, 18, 19, 20], side, invert);

    RiggedHand {
        wrist,
        thumb_proximal: thumb[0],
        thumb_intermediate: thumb[1],
        thumb_distal: thumb[2],
        index_proximal: index[0],
        index_intermediate: index[1],
        index_distal: index[2],
        middle_proximal: middle[0],
        middle_intermediate: middle[1],
        middle_distal: middle[2],
        ring_proximal: ring[0],
        ring_intermediate: ring[1],
        ring_distal: ring[2],
        little_proximal: little[0],
        little_intermediate: little[1],
        little_distal: little[2],
    }
}

/// Calculate thumb finger joints with special dampener/startPos handling.
fn calc_thumb(lm: &[Vec3], side: Side, invert: f32) -> [EulerAngles; 3] {
    // Thumb landmarks: 0, 1, 2, 3, 4
    // proximal = angle(0, 1, 2), intermediate = angle(1, 2, 3), distal = angle(2, 3, 4)
    let angles = [
        angle_between_3d_coords(lm[0], lm[1], lm[2]),
        angle_between_3d_coords(lm[1], lm[2], lm[3]),
        angle_between_3d_coords(lm[2], lm[3], lm[4]),
    ];

    let is_right = side == Side::Right;

    // Proximal: dampener = {x: 2.2, y: 2.2, z: 0.5}, startPos = {x: 1.2, y: 1.1*invert, z: 0.2*invert}
    let proximal = {
        let angle = angles[0];
        let (dx, dy, dz) = (2.2_f32, 2.2_f32, 0.5_f32);
        let (sx, sy, sz) = (1.2_f32, 1.1 * invert, 0.2 * invert);
        EulerAngles {
            x: clamp(sx + angle * -PI * dx, -0.6, 0.3),
            y: clamp(
                sy + angle * -PI * dy * invert,
                if is_right { -1.0 } else { -0.3 },
                if is_right { 0.3 } else { 1.0 },
            ),
            z: clamp(
                sz + angle * -PI * dz * invert,
                if is_right { -0.6 } else { -0.3 },
                if is_right { 0.3 } else { 0.6 },
            ),
        }
    };

    // Intermediate: dampener = {x: 0, y: 0.7, z: 0.5}, startPos = {x: -0.2, y: 0.1*invert, z: 0.2*invert}
    let intermediate = {
        let angle = angles[1];
        let (dx, dy, dz) = (0.0_f32, 0.7_f32, 0.5_f32);
        let (sx, sy, sz) = (-0.2_f32, 0.1 * invert, 0.2 * invert);
        EulerAngles {
            x: clamp(sx + angle * -PI * dx, -2.0, 2.0),
            y: clamp(sy + angle * -PI * dy * invert, -2.0, 2.0),
            z: clamp(sz + angle * -PI * dz * invert, -2.0, 2.0),
        }
    };

    // Distal: dampener = {x: 0, y: 1, z: 0.5}, startPos = {x: -0.2, y: 0.1*invert, z: 0.2*invert}
    let distal = {
        let angle = angles[2];
        let (dx, dy, dz) = (0.0_f32, 1.0_f32, 0.5_f32);
        let (sx, sy, sz) = (-0.2_f32, 0.1 * invert, 0.2 * invert);
        EulerAngles {
            x: clamp(sx + angle * -PI * dx, -2.0, 2.0),
            y: clamp(sy + angle * -PI * dy * invert, -2.0, 2.0),
            z: clamp(sz + angle * -PI * dz * invert, -2.0, 2.0),
        }
    };

    [proximal, intermediate, distal]
}

/// Calculate non-thumb finger joints.
/// indices: [base, mcp, pip, dip, tip] where base is typically 0 (wrist).
fn calc_non_thumb_finger(
    lm: &[Vec3],
    indices: &[usize; 5],
    side: Side,
    invert: f32,
) -> [EulerAngles; 3] {
    // proximal = angle(indices[0], indices[1], indices[2])
    // intermediate = angle(indices[1], indices[2], indices[3])
    // distal = angle(indices[2], indices[3], indices[4])
    let angles = [
        angle_between_3d_coords(lm[indices[0]], lm[indices[1]], lm[indices[2]]),
        angle_between_3d_coords(lm[indices[1]], lm[indices[2]], lm[indices[3]]),
        angle_between_3d_coords(lm[indices[2]], lm[indices[3]], lm[indices[4]]),
    ];

    let is_right = side == Side::Right;

    let mut result = [EulerAngles::default(); 3];
    for i in 0..3 {
        // TypeScript stores bending in z component, then applies:
        // z = clamp(angle * -PI * invert, R ? -PI : 0, R ? 0 : PI)
        let z = clamp(
            angles[i] * -PI * invert,
            if is_right { -PI } else { 0.0 },
            if is_right { 0.0 } else { PI },
        );
        result[i] = EulerAngles { x: 0.0, y: 0.0, z };
    }
    result
}

fn default_hand() -> RiggedHand {
    RiggedHand {
        wrist: EulerAngles::default(),
        thumb_proximal: EulerAngles::default(),
        thumb_intermediate: EulerAngles::default(),
        thumb_distal: EulerAngles::default(),
        index_proximal: EulerAngles::default(),
        index_intermediate: EulerAngles::default(),
        index_distal: EulerAngles::default(),
        middle_proximal: EulerAngles::default(),
        middle_intermediate: EulerAngles::default(),
        middle_distal: EulerAngles::default(),
        ring_proximal: EulerAngles::default(),
        ring_intermediate: EulerAngles::default(),
        ring_distal: EulerAngles::default(),
        little_proximal: EulerAngles::default(),
        little_intermediate: EulerAngles::default(),
        little_distal: EulerAngles::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hand_landmarks() -> Vec<Vec3> {
        let mut lm = vec![Vec3::ZERO; 21];
        lm[0] = Vec3::new(0.0, 0.0, 0.0); // wrist
        lm[5] = Vec3::new(-0.5, 1.0, 0.0); // index base
        lm[17] = Vec3::new(0.5, 1.0, 0.0); // little base
                                           // Fill finger joints with ascending Y positions
        for finger in 0..5 {
            let base = 1 + finger * 4;
            for joint in 0..4 {
                let idx = base + joint;
                let x = -0.5 + (finger as f32) * 0.25;
                let y = 1.0 + (joint as f32) * 0.5;
                lm[idx] = Vec3::new(x, y, 0.0);
            }
        }
        lm
    }

    #[test]
    fn solve_returns_valid_hand() {
        let lm = make_hand_landmarks();
        let hand = solve(&lm, Side::Right);
        assert!(hand.wrist.x.is_finite());
        assert!(hand.wrist.y.is_finite());
        assert!(hand.wrist.z.is_finite());
        assert!(hand.index_proximal.z.is_finite());
    }

    #[test]
    fn solve_insufficient_landmarks_returns_default() {
        let lm = vec![Vec3::ZERO; 10];
        let hand = solve(&lm, Side::Left);
        assert_eq!(hand.wrist.x, 0.0);
        assert_eq!(hand.index_proximal.z, 0.0);
    }

    #[test]
    fn non_thumb_finger_bending_in_z() {
        // Bent finger: segments at 90 degrees
        let mut lm = vec![Vec3::ZERO; 21];
        lm[0] = Vec3::new(0.0, 0.0, 0.0);
        lm[5] = Vec3::new(0.0, 1.0, 0.0);
        lm[6] = Vec3::new(1.0, 1.0, 0.0); // 90 degree bend
        lm[7] = Vec3::new(1.0, 0.0, 0.0);
        lm[8] = Vec3::new(0.0, 0.0, 0.0);
        let result = calc_non_thumb_finger(&lm, &[0, 5, 6, 7, 8], Side::Right, 1.0);
        // Proximal z should be negative for right hand (angle * -PI * 1.0)
        assert!(
            result[0].z < 0.0,
            "expected negative z, got {}",
            result[0].z
        );
        // x and y should be 0
        assert_eq!(result[0].x, 0.0);
        assert_eq!(result[0].y, 0.0);
    }

    #[test]
    fn thumb_produces_finite_values() {
        let lm = make_hand_landmarks();
        let result = calc_thumb(&lm, Side::Right, 1.0);
        for joint in &result {
            assert!(joint.x.is_finite());
            assert!(joint.y.is_finite());
            assert!(joint.z.is_finite());
        }
    }
}
