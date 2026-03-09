use bevy::prelude::*;
use solver::{face, hand, pose, Side, VideoInfo};
use crate::components::landmarks::CurrentLandmarks;
use crate::components::rig::CurrentRig;

/// Compute rig values from landmarks.
pub fn solve_system(
    landmarks: Res<CurrentLandmarks>,
    mut rig: ResMut<CurrentRig>,
) {
    let video = VideoInfo {
        width: 640,
        height: 480,
    };

    if let Some(ref face_lm) = landmarks.face {
        rig.face = Some(face::solve(face_lm, &video));
    }

    if let Some((ref lm3d, ref lm2d)) = landmarks.pose {
        rig.pose = Some(pose::solve(lm3d, lm2d, &video));
    }

    if let Some(ref left) = landmarks.left_hand {
        rig.left_hand = Some(hand::solve(left, Side::Left));
    }
    if let Some(ref right) = landmarks.right_hand {
        rig.right_hand = Some(hand::solve(right, Side::Right));
    }
}
