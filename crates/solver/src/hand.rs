use crate::types::*;
use glam::Vec3;

/// Solve hand rig from 21 hand landmarks.
///
/// Computes wrist rotation and individual finger joint rotations
/// for all 5 fingers (3 joints each = 15 finger joints + wrist).
pub fn solve(landmarks: &[Vec3], side: Side) -> RiggedHand {
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

fn calc_wrist_rotation(lm: &[Vec3], side: Side) -> EulerAngles {
    let _ = (lm, side);
    todo!("Port KalidoKit wrist rotation calculation")
}

fn calc_finger_rotations(lm: &[Vec3], indices: &[usize]) -> [EulerAngles; 3] {
    // Calculate Proximal, Intermediate, Distal rotations from 4 joint positions.
    let _ = (lm, indices);
    todo!("Port KalidoKit finger rotation calculation")
}
