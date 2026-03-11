use std::f32::consts::PI;

/// Clamp a value between min and max.
pub fn clamp(val: f32, min: f32, max: f32) -> f32 {
    val.max(min).min(max)
}

/// Remap a value from one range to another (5-arg version).
pub fn remap(val: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = (val - in_min) / (in_max - in_min);
    out_min + t * (out_max - out_min)
}

/// Remap a value from [min, max] to [0, 1] (KalidoKit 3-arg remap).
/// `remap(val, min, max) = (clamp(val, min, max) - min) / (max - min)`
pub fn remap01(val: f32, min: f32, max: f32) -> f32 {
    let clamped = clamp(val, min, max);
    (clamped - min) / (max - min)
}

/// Linear interpolation between two f32 values.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation between two Vec3 values.
pub fn lerp_vec3(a: glam::Vec3, b: glam::Vec3, t: f32) -> glam::Vec3 {
    a + (b - a) * t
}

/// Angle between two vectors in radians (returns acos(dot)).
/// Used by hand solver.
pub fn angle_between(v1: glam::Vec3, v2: glam::Vec3) -> f32 {
    let dot = v1.normalize().dot(v2.normalize()).clamp(-1.0, 1.0);
    dot.acos()
}

/// Angle between 3D coordinates at vertex b (KalidoKit Vector.angleBetween3DCoords).
/// Returns the cross product magnitude (sin of the angle), NOT the angle itself.
/// Output range: [0, 1] approximately.
pub fn angle_between_3d_coords(a: glam::Vec3, b: glam::Vec3, c: glam::Vec3) -> f32 {
    let v1 = (a - b).normalize();
    let v2 = (c - b).normalize();
    v1.cross(v2).length()
}

/// Quaternion rotation from one direction to another.
pub fn find_rotation_quat(from: glam::Vec3, to: glam::Vec3) -> glam::Quat {
    glam::Quat::from_rotation_arc(from.normalize(), to.normalize())
}

/// 2D angle from (cx, cy) to (ex, ey) — KalidoKit's `find2DAngle`.
pub fn find_2d_angle(cx: f32, cy: f32, ex: f32, ey: f32) -> f32 {
    (ey - cy).atan2(ex - cx)
}

/// Find rotation between two 3D points as Euler-like XYZ (KalidoKit Vector.findRotation).
/// If `normalize` is true, each component is divided by PI to give [-1, 1] range.
pub fn find_rotation(a: glam::Vec3, b: glam::Vec3, normalize: bool) -> glam::Vec3 {
    let x = find_2d_angle(a.z, a.y, b.z, b.y);
    let y = find_2d_angle(a.z, a.x, b.z, b.x);
    let z = find_2d_angle(a.x, a.y, b.x, b.y);
    if normalize {
        glam::Vec3::new(x / PI, y / PI, z / PI)
    } else {
        glam::Vec3::new(x, y, z)
    }
}

/// Roll-pitch-yaw from 2 or 3 points (KalidoKit Vector.rollPitchYaw).
///
/// 2-point mode (c = None): uses find2DAngle between a and b, normalized by PI.
/// 3-point mode (c = Some): forms a plane from a, b, c and computes euler angles.
///
/// Output is normalized: each component divided by PI, giving [-1, 1] range.
pub fn roll_pitch_yaw(a: glam::Vec3, b: glam::Vec3, c: Option<glam::Vec3>) -> glam::Vec3 {
    match c {
        None => {
            // 2-point mode
            let x = find_2d_angle(a.z, a.y, b.z, b.y);
            let y = find_2d_angle(a.z, a.x, b.z, b.x);
            let z = find_2d_angle(a.x, a.y, b.x, b.y);
            glam::Vec3::new(x / PI, y / PI, z / PI)
        }
        Some(c_pt) => {
            // 3-point mode: form plane
            let qb = b - a;
            let qc = c_pt - a;
            let n = qb.cross(qc);
            let len = n.length();
            let unit_n = if len > 1e-10 { n / len } else { n };
            // alpha, beta, gamma
            let x = -(unit_n.x.clamp(-1.0, 1.0).asin());
            let y = unit_n.y.atan2(unit_n.z);
            let z = -(qb.normalize().y.clamp(-1.0, 1.0).asin());
            // Normalize by PI
            glam::Vec3::new(x / PI, y / PI, z / PI)
        }
    }
}

/// Normalize an angle in radians to [-1, 1] range (KalidoKit Vector.normalizeAngle).
pub fn normalize_angle(radians: f32) -> f32 {
    let mut angle = radians % (2.0 * PI);
    if angle > PI {
        angle -= 2.0 * PI;
    }
    if angle < -PI {
        angle += 2.0 * PI;
    }
    angle / PI
}

/// 2D distance (ignoring z component).
pub fn distance2d(a: glam::Vec3, b: glam::Vec3) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

/// 3D distance between two points.
pub fn distance(a: glam::Vec3, b: glam::Vec3) -> f32 {
    (a - b).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(1.5, 0.0, 1.0), 1.0);
        assert_eq!(clamp(-0.5, 0.0, 1.0), 0.0);
        assert_eq!(clamp(0.5, 0.0, 1.0), 0.5);
    }

    #[test]
    fn test_remap() {
        let v = remap(0.5, 0.0, 1.0, 0.0, 10.0);
        assert!((v - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_remap01() {
        // Value in range
        assert!((remap01(0.5, 0.0, 1.0) - 0.5).abs() < 1e-6);
        // Value below range -> clamped to 0
        assert!((remap01(-1.0, 0.0, 1.0) - 0.0).abs() < 1e-6);
        // Value above range -> clamped to 1
        assert!((remap01(2.0, 0.0, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_angle_between() {
        let a = glam::Vec3::X;
        let b = glam::Vec3::Y;
        let angle = angle_between(a, b);
        assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn test_find_rotation_quat() {
        let q = find_rotation_quat(glam::Vec3::X, glam::Vec3::Y);
        let rotated = q * glam::Vec3::X;
        assert!((rotated - glam::Vec3::Y).length() < 1e-5);
    }

    #[test]
    fn test_remap_equal_input_range() {
        let v = remap(5.0, 3.0, 3.0, 0.0, 10.0);
        let _ = v;
    }

    #[test]
    fn test_lerp_vec3() {
        let a = glam::Vec3::ZERO;
        let b = glam::Vec3::new(10.0, 20.0, 30.0);
        let result = lerp_vec3(a, b, 0.5);
        assert!((result.x - 5.0).abs() < 1e-6);
        assert!((result.y - 10.0).abs() < 1e-6);
        assert!((result.z - 15.0).abs() < 1e-6);
    }

    #[test]
    fn test_find_2d_angle() {
        // Angle from origin to (1, 0) should be 0
        assert!((find_2d_angle(0.0, 0.0, 1.0, 0.0) - 0.0).abs() < 1e-6);
        // Angle from origin to (0, 1) should be PI/2
        assert!((find_2d_angle(0.0, 0.0, 0.0, 1.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn test_find_rotation_euler() {
        let a = glam::Vec3::new(0.0, 0.0, 1.0);
        let b = glam::Vec3::new(0.0, 0.0, 2.0);
        let rot = find_rotation(a, b, true);
        // Same x,y but different z -> x and y angles should be 0
        assert!(rot.x.abs() < 1e-5);
        assert!(rot.y.abs() < 1e-5);
    }

    #[test]
    fn test_normalize_angle() {
        assert!((normalize_angle(0.0) - 0.0).abs() < 1e-6);
        assert!((normalize_angle(PI) - 1.0).abs() < 1e-5);
        assert!((normalize_angle(-PI) - (-1.0)).abs() < 1e-5);
        // 2.5*PI wraps to 0.5*PI -> 0.5
        assert!((normalize_angle(2.5 * PI) - 0.5).abs() < 1e-5);
        // -0.5*PI -> -0.5
        assert!((normalize_angle(-0.5 * PI) - (-0.5)).abs() < 1e-5);
    }

    #[test]
    fn test_angle_between_3d_coords() {
        // Right angle at b: a=(1,0,0), b=(0,0,0), c=(0,1,0)
        let val = angle_between_3d_coords(glam::Vec3::X, glam::Vec3::ZERO, glam::Vec3::Y);
        // sin(90deg) = 1.0
        assert!((val - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_roll_pitch_yaw_2point() {
        let a = glam::Vec3::ZERO;
        let b = glam::Vec3::new(1.0, 0.0, 0.0);
        let rpy = roll_pitch_yaw(a, b, None);
        // Each component should be in [-1, 1]
        assert!(rpy.x.abs() <= 1.0);
        assert!(rpy.y.abs() <= 1.0);
        assert!(rpy.z.abs() <= 1.0);
    }

    #[test]
    fn test_roll_pitch_yaw_3point() {
        let a = glam::Vec3::new(0.0, 0.0, 0.0);
        let b = glam::Vec3::new(1.0, 0.0, 0.0);
        let c = glam::Vec3::new(0.0, 1.0, 0.0);
        let rpy = roll_pitch_yaw(a, b, Some(c));
        // Each component should be in [-1, 1]
        assert!(rpy.x.abs() <= 1.0);
        assert!(rpy.y.abs() <= 1.0);
        assert!(rpy.z.abs() <= 1.0);
    }

    #[test]
    fn test_distance2d() {
        let a = glam::Vec3::new(0.0, 0.0, 100.0);
        let b = glam::Vec3::new(3.0, 4.0, 200.0);
        // Should ignore z
        assert!((distance2d(a, b) - 5.0).abs() < 1e-5);
    }
}
