/// Clamp a value between min and max.
pub fn clamp(val: f32, min: f32, max: f32) -> f32 {
    val.max(min).min(max)
}

/// Remap a value from one range to another.
pub fn remap(val: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = (val - in_min) / (in_max - in_min);
    out_min + t * (out_max - out_min)
}

/// Linear interpolation between two f32 values.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation between two Vec3 values.
pub fn lerp_vec3(a: glam::Vec3, b: glam::Vec3, t: f32) -> glam::Vec3 {
    a + (b - a) * t
}

/// Angle between two vectors in radians.
pub fn angle_between(v1: glam::Vec3, v2: glam::Vec3) -> f32 {
    let dot = v1.normalize().dot(v2.normalize()).clamp(-1.0, 1.0);
    dot.acos()
}

/// Quaternion rotation from one direction to another.
pub fn find_rotation(from: glam::Vec3, to: glam::Vec3) -> glam::Quat {
    glam::Quat::from_rotation_arc(from.normalize(), to.normalize())
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
    fn test_find_rotation() {
        let q = find_rotation(glam::Vec3::X, glam::Vec3::Y);
        let rotated = q * glam::Vec3::X;
        assert!((rotated - glam::Vec3::Y).length() < 1e-5);
    }

    #[test]
    fn test_remap_equal_input_range() {
        // in_min == in_max should produce inf/nan but not panic
        let v = remap(5.0, 3.0, 3.0, 0.0, 10.0);
        // Result is inf or nan — just ensure no panic
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
}
