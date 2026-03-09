use crate::types::*;
use crate::utils::{clamp, remap};
use glam::Vec3;

/// Solve face rig from 468+ face landmarks.
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
pub fn stabilize_blink(eye: &EyeValues, head_y: f32) -> EyeValues {
    let max_ratio = 0.285;
    let ratio = clamp(head_y / max_ratio, 0.0, 1.0);
    EyeValues {
        l: eye.l + ratio * (eye.r - eye.l),
        r: eye.r + ratio * (eye.l - eye.r),
    }
}

fn distance(a: Vec3, b: Vec3) -> f32 {
    (a - b).length()
}

fn calc_head_rotation(lm: &[Vec3]) -> EulerAngles {
    if lm.len() < 455 {
        return EulerAngles::default();
    }
    let nose = lm[1];
    let chin = lm[152];
    let left_ear = lm[234];
    let right_ear = lm[454];

    // Vertical: nose to chin vector
    let vertical = (chin - nose).normalize();
    let pitch = vertical.z.atan2(vertical.y);

    // Horizontal: ear to ear vector
    let horizontal = (right_ear - left_ear).normalize();
    let yaw = horizontal.z.atan2(horizontal.x);

    // Roll: tilt based on ear height difference
    let roll = (right_ear.y - left_ear.y).atan2(distance(left_ear, right_ear));

    EulerAngles {
        x: pitch,
        y: yaw,
        z: roll,
    }
}

fn calc_eye_openness(lm: &[Vec3]) -> EyeValues {
    if lm.len() < 387 {
        return EyeValues::default();
    }
    // Left eye: upper=159, lower=145
    let left_dist = distance(lm[159], lm[145]);
    // Right eye: upper=386, lower=374
    let right_dist = distance(lm[386], lm[374]);

    // Normalize by inter-eye distance
    let eye_width = distance(lm[33], lm[133]).max(0.001);

    let l = remap(left_dist / eye_width, 0.15, 0.45, 0.0, 1.0).clamp(0.0, 1.0);
    let r = remap(right_dist / eye_width, 0.15, 0.45, 0.0, 1.0).clamp(0.0, 1.0);

    EyeValues { l, r }
}

fn calc_mouth_shape(lm: &[Vec3]) -> MouthShape {
    if lm.len() < 309 {
        return MouthShape::default();
    }
    // Mouth open: upper lip (13) to lower lip (14)
    let mouth_open = distance(lm[13], lm[14]);
    // Mouth width: left corner (78) to right corner (308)
    let mouth_width = distance(lm[78], lm[308]);

    // Normalize by face height (nose to chin)
    let face_height = distance(lm[1], lm[152]).max(0.001);

    let open_ratio = mouth_open / face_height;
    let width_ratio = mouth_width / face_height;

    // Map to vowel shapes (simplified KalidoKit mapping)
    let a = remap(open_ratio, 0.02, 0.12, 0.0, 1.0).clamp(0.0, 1.0);
    let i = remap(width_ratio, 0.3, 0.5, 0.0, 1.0).clamp(0.0, 1.0) * (1.0 - a);
    let u = remap(width_ratio, 0.1, 0.25, 1.0, 0.0).clamp(0.0, 1.0) * a.min(0.5);
    let e = remap(open_ratio, 0.04, 0.08, 0.0, 1.0).clamp(0.0, 1.0) * (1.0 - a) * i;
    let o = a * remap(width_ratio, 0.2, 0.35, 1.0, 0.0).clamp(0.0, 1.0);

    MouthShape { a, i, u, e, o }
}

fn calc_pupil_position(lm: &[Vec3]) -> glam::Vec2 {
    if lm.len() < 478 {
        return glam::Vec2::ZERO;
    }
    // Iris landmarks: left eye center (468), right eye center (473)
    let left_iris = lm[468];
    let right_iris = lm[473];

    // Eye corners for normalization
    let left_outer = lm[33];
    let left_inner = lm[133];
    let right_outer = lm[362];
    let right_inner = lm[263];

    // Calculate horizontal offset
    let left_x =
        remap_eye_position(left_iris.x, left_outer.x, left_inner.x);
    let right_x =
        remap_eye_position(right_iris.x, right_outer.x, right_inner.x);
    let x = (left_x + right_x) * 0.5;

    // Calculate vertical offset
    let left_y = remap_eye_position(left_iris.y, lm[159].y, lm[145].y);
    let right_y = remap_eye_position(right_iris.y, lm[386].y, lm[374].y);
    let y = (left_y + right_y) * 0.5;

    glam::Vec2::new(x, y)
}

fn remap_eye_position(iris: f32, outer: f32, inner: f32) -> f32 {
    let range = (inner - outer).abs().max(0.001);
    let normalized = (iris - outer) / range;
    (normalized * 2.0 - 1.0).clamp(-1.0, 1.0)
}

fn calc_brow_raise(lm: &[Vec3]) -> f32 {
    if lm.len() < 300 {
        return 0.0;
    }
    // Brow landmarks: left brow (105), right brow (334)
    // Eye landmarks: left eye top (159), right eye top (386)
    let left_brow_dist = distance(lm[105], lm[159]);
    let right_brow_dist = distance(lm[334], lm[386]);
    let avg_dist = (left_brow_dist + right_brow_dist) * 0.5;

    let face_height = distance(lm[1], lm[152]).max(0.001);
    let ratio = avg_dist / face_height;

    remap(ratio, 0.06, 0.12, 0.0, 1.0).clamp(0.0, 1.0)
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
    fn stabilize_blink_compensates() {
        let eye = EyeValues { l: 0.8, r: 0.4 };
        // At half head rotation, stabilization partially blends values
        let stabilized = stabilize_blink(&eye, 0.285 * 0.5);
        // l should decrease, r should increase (partial blend)
        assert!(stabilized.l < eye.l);
        assert!(stabilized.r > eye.r);
    }
}
