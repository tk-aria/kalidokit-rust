use crate::types::*;
use crate::utils::{clamp, distance, distance2d, lerp, lerp_vec3, remap01, roll_pitch_yaw};
use glam::Vec3;
use std::f32::consts::PI;

/// Solve face rig from 468+ face landmarks.
///
/// Landmarks are expected as normalized [0,1] coordinates from MediaPipe.
/// They are scaled to pixel coordinates using `video` dimensions internally,
/// matching the original KalidoKit behavior.
pub fn solve(landmarks: &[Vec3], video: &VideoInfo) -> RiggedFace {
    // Scale normalized landmarks to pixel coordinates (KalidoKit convention).
    // x/y are normalized [0,1] from tracker -> scale to pixel coords.
    // z is already in raw pixel scale from normalize_landmarks (not divided by width),
    // so it does NOT need to be scaled again.
    let lm: Vec<Vec3> = landmarks
        .iter()
        .map(|p| {
            Vec3::new(
                p.x * video.width as f32,
                p.y * video.height as f32,
                p.z, // z is already in pixel-scale from ONNX model output
            )
        })
        .collect();

    let head = calc_head_rotation(&lm);
    let eye = calc_eyes(&lm);
    let mouth = calc_mouth(&lm);
    let pupil = calc_pupil_position(&lm);
    let brow = calc_brow_raise(&lm);

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
/// Matches KalidoKit's stabilizeBlink with enableWink=true.
pub fn stabilize_blink(eye: &EyeValues, head_y: f32) -> EyeValues {
    let l = clamp(eye.l, 0.0, 1.0);
    let r = clamp(eye.r, 0.0, 1.0);
    let blink_diff = (l - r).abs();
    let blink_thresh = 0.8;
    let max_rot = 0.5;
    let is_closing = l < 0.3 && r < 0.3;
    let is_open = l > 0.6 && r > 0.6;

    // Head turned far right -> use right eye for both
    if head_y > max_rot {
        return EyeValues { l: r, r };
    }
    // Head turned far left -> use left eye for both
    if head_y < -max_rot {
        return EyeValues { l, r: l };
    }

    if blink_diff >= blink_thresh && !is_closing && !is_open {
        // Wink detected: keep individual values
        EyeValues { l, r }
    } else {
        // Blend eyes together for stability
        if r > l {
            EyeValues {
                l: lerp(r, l, 0.95),
                r: lerp(r, l, 0.05),
            }
        } else {
            EyeValues {
                l: lerp(r, l, 0.05),
                r: lerp(r, l, 0.95),
            }
        }
    }
}

/// Head rotation using KalidoKit's createEulerPlane algorithm.
///
/// Uses landmarks 21 (top-left), 251 (top-right), 397 (bottom-right), 172 (bottom-left).
fn calc_head_rotation(lm: &[Vec3]) -> EulerAngles {
    if lm.len() < 455 {
        return EulerAngles::default();
    }

    // createEulerPlane: midpoint of bottom-right and bottom-left
    let p3mid = lerp_vec3(lm[397], lm[172], 0.5);
    // Roll-pitch-yaw from plane [lm[21], lm[251], p3mid]
    let mut rotation = roll_pitch_yaw(lm[21], lm[251], Some(p3mid));

    // KalidoKit applies these sign flips
    rotation.x *= -1.0;
    rotation.z *= -1.0;

    // Output is rotation * PI (converting from [-1,1] back to radians)
    EulerAngles {
        x: rotation.x * PI,
        y: rotation.y * PI,
        z: rotation.z * PI,
    }
}

/// Eye lid ratio: average vertical lid distance / eye width.
///
/// Takes 8 points per eye in order:
/// [outerCorner, innerCorner, outerUpperLid, midUpperLid, innerUpperLid,
///  outerLowerLid, midLowerLid, innerLowerLid]
fn eye_lid_ratio(points: &[Vec3; 8]) -> f32 {
    let outer_corner = points[0];
    let inner_corner = points[1];
    let outer_upper = points[2];
    let mid_upper = points[3];
    let inner_upper = points[4];
    let outer_lower = points[5];
    let mid_lower = points[6];
    let inner_lower = points[7];

    let eye_width = distance2d(outer_corner, inner_corner).max(0.001);
    let outer_lid_dist = distance2d(outer_upper, outer_lower);
    let mid_lid_dist = distance2d(mid_upper, mid_lower);
    let inner_lid_dist = distance2d(inner_upper, inner_lower);

    let avg = (outer_lid_dist + mid_lid_dist + inner_lid_dist) / 3.0;
    avg / eye_width
}

/// Calculate eye openness using KalidoKit's calcEyes algorithm.
fn calc_eyes(lm: &[Vec3]) -> EyeValues {
    // Need at least 468 landmarks (standard face mesh) for eye detection.
    // KalidoKit requires 478 (with iris), but eye lid ratio works with 468.
    if lm.len() < 468 {
        return EyeValues { l: 1.0, r: 1.0 };
    }

    // Left eye points: [130, 133, 160, 159, 158, 144, 145, 153]
    let left_points: [Vec3; 8] = [
        lm[130], lm[133], lm[160], lm[159], lm[158], lm[144], lm[145], lm[153],
    ];
    // Right eye points: [263, 362, 387, 386, 385, 373, 374, 380]
    let right_points: [Vec3; 8] = [
        lm[263], lm[362], lm[387], lm[386], lm[385], lm[373], lm[374], lm[380],
    ];

    let left_ratio = eye_lid_ratio(&left_points);
    let right_ratio = eye_lid_ratio(&right_points);

    let max_ratio = 0.285;

    let left_clamped = clamp(left_ratio / max_ratio, 0.0, 2.0);
    let right_clamped = clamp(right_ratio / max_ratio, 0.0, 2.0);

    let l = remap01(left_clamped, 0.35, 0.5);
    let r = remap01(right_clamped, 0.35, 0.5);

    EyeValues { l, r }
}

/// Calculate mouth shape using KalidoKit's calcMouth algorithm.
///
/// Uses eye distances as reference scale (matching the reference testbed).
/// Derives vowel shapes from mouth openness (y) and width (x).
fn calc_mouth(lm: &[Vec3]) -> MouthShape {
    if lm.len() < 468 {
        return MouthShape::default();
    }

    // Eye reference distances
    let eye_inner_distance = distance(lm[133], lm[362]).max(0.001);
    let eye_outer_distance = distance(lm[130], lm[263]).max(0.001);

    // Mouth landmarks
    let upper_lip = lm[13];
    let lower_lip = lm[14];
    let mouth_corner_l = lm[61];
    let mouth_corner_r = lm[291];

    let mouth_open = distance(upper_lip, lower_lip);
    let mouth_width = distance(mouth_corner_l, mouth_corner_r);

    // KalidoKit original ratios
    let _ratio_y = remap01(mouth_open / eye_inner_distance, 0.15, 0.7);
    let ratio_x = remap01(mouth_width / eye_outer_distance, 0.45, 0.9);
    let ratio_x = (ratio_x - 0.3) * 2.0;

    let mouth_x = ratio_x;
    let raw_mouth_y = remap01(mouth_open / eye_inner_distance, 0.17, 0.5);

    // Compress large openings and cap at 70% to avoid over-exaggeration
    let mouth_y = (raw_mouth_y.sqrt() * raw_mouth_y.sqrt().sqrt()) * 0.7; // ~pow(0.75) * 0.7

    // KalidoKit vowel shape formulas
    let ratio_i = clamp(
        remap01(mouth_x, 0.0, 1.0) * 2.0 * remap01(mouth_y, 0.2, 0.7),
        0.0,
        1.0,
    );

    // Shift toward O when mouth is wide open (rounder, cuter look)
    let open_factor = remap01(raw_mouth_y, 0.3, 0.7); // how "wide open" the mouth is

    // Suppress I (horizontal stretch) when wide open to avoid diamond shape
    let ratio_i_dampened = ratio_i * (1.0 - open_factor * 0.8);

    // A: reduce significantly when wide open, letting O take over
    let a = mouth_y * 0.3 + mouth_y * (1.0 - ratio_i_dampened) * 0.4 * (1.0 - open_factor * 0.7);
    let u = mouth_y * remap01(1.0 - ratio_i_dampened, 0.0, 0.3) * 0.1;
    let e = remap01(u, 0.2, 1.0) * (1.0 - ratio_i_dampened) * 0.3;
    // O: dominant when wide open for round shape
    let o = (1.0 - ratio_i_dampened) * remap01(mouth_y, 0.15, 0.6) * (0.5 + open_factor * 0.5);

    MouthShape {
        a,
        i: ratio_i_dampened,
        u,
        e,
        o,
    }
}

/// Calculate pupil position using KalidoKit's algorithm.
fn calc_pupil_position(lm: &[Vec3]) -> glam::Vec2 {
    // Pupil tracking requires iris landmarks (468-477), so need 478 total
    if lm.len() < 478 {
        // Fall back to center gaze when iris landmarks not available
        return glam::Vec2::ZERO;
    }

    // Left eye
    let l_outer = lm[130];
    let l_inner = lm[133];
    let l_eye_width = distance2d(l_outer, l_inner).max(0.001);
    let l_mid = lerp_vec3(l_outer, l_inner, 0.5);
    let l_pupil = lm[468];
    let l_dx = l_mid.x - l_pupil.x;
    let l_dy = l_mid.y - l_eye_width * 0.075 - l_pupil.y;
    let l_ratio_x = l_dx / (l_eye_width / 2.0) * 4.0;
    let l_ratio_y = l_dy / (l_eye_width / 4.0) * 4.0;

    // Right eye
    let r_outer = lm[263];
    let r_inner = lm[362];
    let r_eye_width = distance2d(r_outer, r_inner).max(0.001);
    let r_mid = lerp_vec3(r_outer, r_inner, 0.5);
    let r_pupil = lm[473];
    let r_dx = r_mid.x - r_pupil.x;
    let r_dy = r_mid.y - r_eye_width * 0.075 - r_pupil.y;
    let r_ratio_x = r_dx / (r_eye_width / 2.0) * 4.0;
    let r_ratio_y = r_dy / (r_eye_width / 4.0) * 4.0;

    // Average left and right
    let x = (l_ratio_x + r_ratio_x) * 0.5;
    let y = (l_ratio_y + r_ratio_y) * 0.5;

    glam::Vec2::new(x, y)
}

/// Calculate brow raise using KalidoKit's eyeLidRatio on brow landmarks.
fn calc_brow_raise(lm: &[Vec3]) -> f32 {
    if lm.len() < 468 {
        return 0.0;
    }

    // Left brow points: [35, 244, 63, 105, 66, 229, 230, 231]
    let left_brow_points: [Vec3; 8] = [
        lm[35], lm[244], lm[63], lm[105], lm[66], lm[229], lm[230], lm[231],
    ];
    // Right brow points: [265, 464, 293, 334, 296, 449, 450, 451]
    let right_brow_points: [Vec3; 8] = [
        lm[265], lm[464], lm[293], lm[334], lm[296], lm[449], lm[450], lm[451],
    ];

    let left_brow_dist = eye_lid_ratio(&left_brow_points);
    let right_brow_dist = eye_lid_ratio(&right_brow_points);
    let avg_brow_dist = (left_brow_dist + right_brow_dist) * 0.5;

    let max_brow_ratio = 1.15;
    let brow_high = 0.125;
    let brow_low = 0.07;

    let brow_ratio = avg_brow_dist / max_brow_ratio - 1.0;
    let clamped = clamp(brow_ratio, brow_low, brow_high);
    (clamped - brow_low) / (brow_high - brow_low)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dummy_landmarks(count: usize) -> Vec<Vec3> {
        (0..count)
            .map(|i| Vec3::new(i as f32 * 0.01, i as f32 * 0.005, 0.0))
            .collect()
    }

    #[test]
    fn solve_with_enough_landmarks() {
        let lm = make_dummy_landmarks(478);
        let video = VideoInfo {
            width: 640,
            height: 480,
        };
        let result = solve(&lm, &video);
        // Should not panic
        assert!(!result.head.x.is_nan());
    }

    #[test]
    fn solve_with_too_few_landmarks_returns_defaults() {
        let lm = make_dummy_landmarks(10);
        let video = VideoInfo {
            width: 640,
            height: 480,
        };
        let result = solve(&lm, &video);
        assert_eq!(result.brow, 0.0);
    }

    #[test]
    fn stabilize_blink_wink_detection() {
        // Large difference between eyes -> wink (diff=0.9 > thresh=0.8)
        let eye = EyeValues { l: 1.0, r: 0.1 };
        let stabilized = stabilize_blink(&eye, 0.0);
        // With blink_diff >= 0.8, wink mode: values should be preserved
        assert!((stabilized.l - 1.0).abs() < 1e-6);
        assert!((stabilized.r - 0.1).abs() < 1e-6);
    }

    #[test]
    fn stabilize_blink_head_turned_right() {
        let eye = EyeValues { l: 0.8, r: 0.4 };
        let stabilized = stabilize_blink(&eye, 0.6); // > max_rot (0.5)
                                                     // Should use right eye for both
        assert!((stabilized.l - 0.4).abs() < 1e-6);
        assert!((stabilized.r - 0.4).abs() < 1e-6);
    }

    #[test]
    fn stabilize_blink_head_turned_left() {
        let eye = EyeValues { l: 0.8, r: 0.4 };
        let stabilized = stabilize_blink(&eye, -0.6); // < -max_rot
                                                      // Should use left eye for both
        assert!((stabilized.l - 0.8).abs() < 1e-6);
        assert!((stabilized.r - 0.8).abs() < 1e-6);
    }

    #[test]
    fn stabilize_blink_blending() {
        // Normal case: small difference, blend together
        let eye = EyeValues { l: 0.7, r: 0.6 };
        let stabilized = stabilize_blink(&eye, 0.0);
        // Values should be blended toward each other
        assert!(stabilized.l > 0.6 && stabilized.l < 0.71);
        assert!(stabilized.r > 0.59 && stabilized.r < 0.7);
    }

    #[test]
    fn head_rotation_facing_forward_near_zero() {
        // Create symmetric face landmarks at pixel scale
        let mut lm = vec![Vec3::ZERO; 478];
        // Landmarks 21 and 251 at same height (top-left and top-right)
        lm[21] = Vec3::new(200.0, 100.0, 0.0);
        lm[251] = Vec3::new(440.0, 100.0, 0.0);
        // Landmarks 397 and 172 at same height (bottom-right and bottom-left)
        lm[397] = Vec3::new(440.0, 380.0, 0.0);
        lm[172] = Vec3::new(200.0, 380.0, 0.0);
        let head = calc_head_rotation(&lm);
        // Roll should be near zero for symmetric face
        assert!(
            head.z.abs() < 0.3,
            "roll should be near zero, got {}",
            head.z
        );
    }

    #[test]
    fn eyes_return_valid_range() {
        let mut lm = vec![Vec3::ZERO; 478];
        // Set up left eye points with reasonable spacing
        // [130, 133, 160, 159, 158, 144, 145, 153]
        lm[130] = Vec3::new(100.0, 200.0, 0.0); // outer corner
        lm[133] = Vec3::new(150.0, 200.0, 0.0); // inner corner
        lm[160] = Vec3::new(110.0, 190.0, 0.0); // outer upper
        lm[159] = Vec3::new(125.0, 188.0, 0.0); // mid upper
        lm[158] = Vec3::new(140.0, 190.0, 0.0); // inner upper
        lm[144] = Vec3::new(110.0, 210.0, 0.0); // outer lower
        lm[145] = Vec3::new(125.0, 212.0, 0.0); // mid lower
        lm[153] = Vec3::new(140.0, 210.0, 0.0); // inner lower
                                                // Right eye points [263, 362, 387, 386, 385, 373, 374, 380]
        lm[263] = Vec3::new(250.0, 200.0, 0.0);
        lm[362] = Vec3::new(300.0, 200.0, 0.0);
        lm[387] = Vec3::new(260.0, 190.0, 0.0);
        lm[386] = Vec3::new(275.0, 188.0, 0.0);
        lm[385] = Vec3::new(290.0, 190.0, 0.0);
        lm[373] = Vec3::new(260.0, 210.0, 0.0);
        lm[374] = Vec3::new(275.0, 212.0, 0.0);
        lm[380] = Vec3::new(290.0, 210.0, 0.0);
        let eye = calc_eyes(&lm);
        assert!(
            eye.l >= 0.0 && eye.l <= 1.0,
            "left eye out of range: {}",
            eye.l
        );
        assert!(
            eye.r >= 0.0 && eye.r <= 1.0,
            "right eye out of range: {}",
            eye.r
        );
    }

    #[test]
    fn mouth_closed_low_values() {
        let mut lm = vec![Vec3::ZERO; 478];
        // Eye reference landmarks
        lm[133] = Vec3::new(150.0, 200.0, 0.0);
        lm[362] = Vec3::new(300.0, 200.0, 0.0);
        lm[130] = Vec3::new(100.0, 200.0, 0.0);
        lm[263] = Vec3::new(350.0, 200.0, 0.0);
        // Close lips
        lm[13] = Vec3::new(225.0, 300.0, 0.0);
        lm[14] = Vec3::new(225.0, 302.0, 0.0);
        // Mouth corners
        lm[61] = Vec3::new(180.0, 301.0, 0.0);
        lm[291] = Vec3::new(270.0, 301.0, 0.0);
        let mouth = calc_mouth(&lm);
        assert!(
            mouth.a < 0.3,
            "mouth.a should be low for closed mouth, got {}",
            mouth.a
        );
    }
}
