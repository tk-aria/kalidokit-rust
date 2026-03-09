use crate::types::*;
use crate::utils::clamp;
use glam::Vec3;

/// Solve face rig from 468 face landmarks.
///
/// Computes head rotation, eye blink values, mouth vowel shapes,
/// pupil direction, and brow raise amount.
pub fn solve(landmarks: &[Vec3], _video: &VideoInfo) -> RiggedFace {
    let head = calc_head_rotation(landmarks);
    let eye = calc_eye_openness(landmarks);
    let mouth = calc_mouth_shape(landmarks);
    let pupil = calc_pupil_position(landmarks);
    let brow = calc_brow_raise(landmarks);

    RiggedFace {
        head,
        eye,
        pupil,
        mouth,
        brow,
    }
}

/// Stabilize blink values based on head Y rotation.
///
/// When the head is turned sideways, one eye may appear more closed
/// due to perspective. This function compensates for that effect.
pub fn stabilize_blink(eye: &EyeValues, head_y: f32) -> EyeValues {
    let max_ratio = 0.285;
    let ratio = clamp(head_y / max_ratio, 0.0, 1.0);
    EyeValues {
        l: eye.l + ratio * (eye.r - eye.l),
        r: eye.r + ratio * (eye.l - eye.r),
    }
}

fn calc_head_rotation(lm: &[Vec3]) -> EulerAngles {
    // Estimate head rotation from nose tip, chin, and ear landmarks.
    // Uses landmarks: nose tip (1), chin (152), left ear (234), right ear (454)
    let _ = lm;
    todo!("Port KalidoKit head rotation calculation")
}

fn calc_eye_openness(lm: &[Vec3]) -> EyeValues {
    // Calculate eye openness from upper/lower eyelid landmark distance.
    let _ = lm;
    todo!("Port KalidoKit eye openness calculation")
}

fn calc_mouth_shape(lm: &[Vec3]) -> MouthShape {
    // Estimate Japanese vowel shapes (A/I/U/E/O) from mouth landmarks.
    let _ = lm;
    todo!("Port KalidoKit mouth shape calculation")
}

fn calc_pupil_position(lm: &[Vec3]) -> glam::Vec2 {
    // Calculate pupil position from iris landmarks (indices 468-477).
    let _ = lm;
    todo!("Port KalidoKit pupil position calculation")
}

fn calc_brow_raise(lm: &[Vec3]) -> f32 {
    // Calculate brow raise amount from eyebrow landmark heights.
    let _ = lm;
    todo!("Port KalidoKit brow raise calculation")
}
