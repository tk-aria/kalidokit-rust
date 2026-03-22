use glam::Vec3;

/// Constrain tail position to maintain bone length from center.
///
/// Returns the adjusted tail position that is exactly `bone_length` away from `center`.
/// If tail is at the same position as center (zero direction), falls back to +Y direction.
pub fn length_constraint(tail: Vec3, center: Vec3, bone_length: f32) -> Vec3 {
    let dir = tail - center;
    let len = dir.length();
    if len > 1e-6 {
        center + (dir / len) * bone_length
    } else {
        // Fallback: push in +Y direction when tail is at center
        center + Vec3::Y * bone_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintains_exact_bone_length() {
        let center = Vec3::ZERO;
        let tail = Vec3::new(3.0, 0.0, 0.0);
        let bone_length = 2.0;
        let result = length_constraint(tail, center, bone_length);
        let distance = (result - center).length();
        assert!(
            (distance - bone_length).abs() < 1e-6,
            "expected distance {bone_length}, got {distance}"
        );
    }

    #[test]
    fn stretched_tail_pulled_back() {
        let center = Vec3::new(1.0, 1.0, 1.0);
        let tail = Vec3::new(11.0, 1.0, 1.0); // 10 units away
        let bone_length = 2.0;
        let result = length_constraint(tail, center, bone_length);
        let distance = (result - center).length();
        assert!(
            (distance - bone_length).abs() < 1e-6,
            "stretched tail should be pulled back to {bone_length}, got {distance}"
        );
        // Direction should be preserved (+X)
        assert!((result.x - 3.0).abs() < 1e-6);
        assert!((result.y - 1.0).abs() < 1e-6);
        assert!((result.z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn compressed_tail_pushed_out() {
        let center = Vec3::ZERO;
        let tail = Vec3::new(0.1, 0.0, 0.0); // very close
        let bone_length = 5.0;
        let result = length_constraint(tail, center, bone_length);
        let distance = (result - center).length();
        assert!(
            (distance - bone_length).abs() < 1e-6,
            "compressed tail should be pushed out to {bone_length}, got {distance}"
        );
        // Direction should be preserved (+X)
        assert!(result.x > 0.0);
    }

    #[test]
    fn tail_at_center_returns_fallback_direction() {
        let center = Vec3::new(2.0, 3.0, 4.0);
        let tail = center; // exactly at center
        let bone_length = 1.5;
        let result = length_constraint(tail, center, bone_length);
        let distance = (result - center).length();
        assert!(
            (distance - bone_length).abs() < 1e-6,
            "fallback should maintain bone_length, got {distance}"
        );
        // Should fall back to +Y direction
        let expected = center + Vec3::Y * bone_length;
        assert!(
            (result - expected).length() < 1e-6,
            "expected +Y fallback at {expected}, got {result}"
        );
    }
}
