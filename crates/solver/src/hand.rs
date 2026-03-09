use crate::types::*;
use crate::utils::angle_between;
use glam::Vec3;

/// Solve hand rig from 21 hand landmarks.
///
/// Computes wrist rotation and individual finger joint rotations
/// for all 5 fingers (3 joints each = 15 finger joints + wrist).
pub fn solve(landmarks: &[Vec3], side: Side) -> RiggedHand {
    if landmarks.len() < 21 {
        return default_hand();
    }

    let wrist = calc_wrist_rotation(landmarks, side);

    // MediaPipe hand landmark indices per finger:
    // Thumb:  1, 2, 3, 4
    // Index:  5, 6, 7, 8
    // Middle: 9, 10, 11, 12
    // Ring:   13, 14, 15, 16
    // Little: 17, 18, 19, 20

    let thumb = calc_finger_rotations(landmarks, &[1, 2, 3, 4]);
    let index = calc_finger_rotations(landmarks, &[5, 6, 7, 8]);
    let middle = calc_finger_rotations(landmarks, &[9, 10, 11, 12]);
    let ring = calc_finger_rotations(landmarks, &[13, 14, 15, 16]);
    let little = calc_finger_rotations(landmarks, &[17, 18, 19, 20]);

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

/// Calculate wrist rotation from landmarks 0 (wrist), 5 (index base), 17 (little base).
fn calc_wrist_rotation(lm: &[Vec3], side: Side) -> EulerAngles {
    let wrist = lm[0];
    let index_base = lm[5];
    let little_base = lm[17];

    // Palm forward direction: wrist → middle of index/little bases
    let palm_center = (index_base + little_base) * 0.5;
    let forward = (palm_center - wrist).normalize_or_zero();

    // Palm lateral direction: index base → little base (cross-palm)
    let lateral = (little_base - index_base).normalize_or_zero();

    // Palm normal via cross product
    let normal = forward.cross(lateral).normalize_or_zero();

    // Compute euler angles from forward direction
    let yaw = forward.x.atan2(forward.z);
    let pitch = (-forward.y)
        .asin()
        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    let roll = normal.x.atan2(normal.y);

    // Mirror yaw for left hand
    let side_sign = match side {
        Side::Right => 1.0,
        Side::Left => -1.0,
    };

    EulerAngles {
        x: pitch,
        y: yaw * side_sign,
        z: roll * side_sign,
    }
}

/// Calculate Proximal, Intermediate, Distal rotations from 4 joint positions.
fn calc_finger_rotations(lm: &[Vec3], indices: &[usize]) -> [EulerAngles; 3] {
    let joints: Vec<Vec3> = indices.iter().map(|&i| lm[i]).collect();
    let mut result = [EulerAngles::default(); 3];
    for i in 0..3 {
        let v1 = (joints[i + 1] - joints[i]).normalize_or_zero();
        let v2 = if i + 2 < joints.len() {
            (joints[i + 2] - joints[i + 1]).normalize_or_zero()
        } else {
            v1
        };
        result[i] = EulerAngles {
            x: angle_between(v1, v2),
            y: 0.0,
            z: 0.0,
        };
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
        // 21 landmarks: wrist at origin, fingers extending along +Y
        let mut lm = vec![Vec3::ZERO; 21];
        lm[0] = Vec3::new(0.0, 0.0, 0.0); // wrist
                                          // Index base and little base for wrist rotation
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
        // All angles should be finite
        assert!(hand.wrist.x.is_finite());
        assert!(hand.wrist.y.is_finite());
        assert!(hand.wrist.z.is_finite());
        assert!(hand.index_proximal.x.is_finite());
    }

    #[test]
    fn solve_insufficient_landmarks_returns_default() {
        let lm = vec![Vec3::ZERO; 10]; // Not enough
        let hand = solve(&lm, Side::Left);
        assert_eq!(hand.wrist.x, 0.0);
        assert_eq!(hand.index_proximal.x, 0.0);
    }

    #[test]
    fn finger_rotations_straight_finger_zero_angle() {
        // Straight finger: all joints in a line along +Y
        let lm = vec![
            Vec3::ZERO,
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
        ];
        let rotations = calc_finger_rotations(&lm, &[1, 2, 3, 4]);
        // Straight finger → angle between consecutive segments ≈ 0
        for r in &rotations {
            assert!(r.x.abs() < 1e-5, "expected near-zero angle, got {}", r.x);
        }
    }

    #[test]
    fn finger_rotations_bent_finger_nonzero() {
        // Bent finger: segments at 90 degrees
        let lm = vec![
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 0.0), // joint 0
            Vec3::new(0.0, 1.0, 0.0), // joint 1 (+Y)
            Vec3::new(1.0, 1.0, 0.0), // joint 2 (+X, 90deg bend)
            Vec3::new(1.0, 0.0, 0.0), // joint 3 (-Y, another 90deg)
        ];
        let rotations = calc_finger_rotations(&lm, &[1, 2, 3, 4]);
        // First joint: angle between (0→1) and (1→2) should be ~90deg
        let expected = std::f32::consts::FRAC_PI_2;
        assert!(
            (rotations[0].x - expected).abs() < 0.1,
            "expected ~{}, got {}",
            expected,
            rotations[0].x
        );
    }
}
