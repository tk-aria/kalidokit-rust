use glam::Vec3;

use crate::config::SpringConfig;

/// Maximum delta time to prevent physics explosion.
const MAX_DT: f32 = 0.05;

/// Perform one Verlet integration step.
///
/// Computes the next tail position by combining inertia (velocity),
/// stiffness restoring force, gravity, and wind.
///
/// `dt` is clamped to `[0.0, MAX_DT]` to prevent instability.
pub fn verlet_step(
    current: Vec3,
    prev: Vec3,
    config: &SpringConfig,
    initial_world_tail: Vec3,
    _center: Vec3,
    wind: Vec3,
    dt: f32,
) -> Vec3 {
    let dt = dt.clamp(0.0, MAX_DT);
    let velocity = (current - prev) * (1.0 - config.drag_force);
    let stiffness_force =
        (initial_world_tail - current).normalize_or_zero() * config.stiffness * dt;
    let gravity = config.gravity_dir * config.gravity_power * dt;
    let wind_force = wind * config.wind_scale * dt;
    current + velocity + stiffness_force + gravity + wind_force
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> SpringConfig {
        SpringConfig::default()
    }

    #[test]
    fn zero_drag_preserves_velocity() {
        let config = SpringConfig {
            drag_force: 0.0,
            stiffness: 0.0,
            gravity_power: 0.0,
            wind_scale: 0.0,
            ..default_config()
        };
        let prev = Vec3::ZERO;
        let current = Vec3::new(1.0, 0.0, 0.0);
        let result = verlet_step(current, prev, &config, current, Vec3::ZERO, Vec3::ZERO, 0.016);
        // velocity = (1,0,0) - (0,0,0) = (1,0,0), no drag → full velocity preserved
        // result = current + velocity = (2,0,0)
        assert!((result.x - 2.0).abs() < 1e-5);
        assert!(result.y.abs() < 1e-5);
    }

    #[test]
    fn full_drag_stops_movement() {
        let config = SpringConfig {
            drag_force: 1.0,
            stiffness: 0.0,
            gravity_power: 0.0,
            wind_scale: 0.0,
            ..default_config()
        };
        let prev = Vec3::ZERO;
        let current = Vec3::new(1.0, 0.0, 0.0);
        let result = verlet_step(current, prev, &config, current, Vec3::ZERO, Vec3::ZERO, 0.016);
        // velocity = (1,0,0) * (1 - 1.0) = (0,0,0)
        // result = current + 0 = current (no stiffness/gravity/wind)
        assert!((result - current).length() < 1e-5);
    }

    #[test]
    fn gravity_pulls_downward() {
        let config = SpringConfig {
            drag_force: 1.0,
            stiffness: 0.0,
            gravity_power: 1.0,
            gravity_dir: Vec3::new(0.0, -1.0, 0.0),
            wind_scale: 0.0,
            ..default_config()
        };
        let pos = Vec3::new(0.0, 5.0, 0.0);
        let result = verlet_step(pos, pos, &config, pos, Vec3::ZERO, Vec3::ZERO, 0.016);
        // Only gravity applies: y should decrease
        assert!(result.y < pos.y);
    }

    #[test]
    fn stiffness_restores_toward_initial() {
        let config = SpringConfig {
            drag_force: 1.0,
            stiffness: 2.0,
            gravity_power: 0.0,
            wind_scale: 0.0,
            ..default_config()
        };
        let initial = Vec3::new(0.0, 3.0, 0.0);
        let current = Vec3::new(0.0, 1.0, 0.0); // displaced downward from initial
        let result =
            verlet_step(current, current, &config, initial, Vec3::ZERO, Vec3::ZERO, 0.016);
        // Stiffness should pull current toward initial (upward)
        assert!(result.y > current.y);
    }

    #[test]
    fn wind_adds_force() {
        let config = SpringConfig {
            drag_force: 1.0,
            stiffness: 0.0,
            gravity_power: 0.0,
            wind_scale: 1.0,
            ..default_config()
        };
        let pos = Vec3::ZERO;
        let wind = Vec3::new(1.0, 0.0, 0.0);
        let result = verlet_step(pos, pos, &config, pos, Vec3::ZERO, wind, 0.016);
        // Wind pushes in +x direction
        assert!(result.x > 0.0);
        assert!(result.y.abs() < 1e-5);
    }

    #[test]
    fn large_dt_clamped() {
        let config = SpringConfig {
            drag_force: 0.0,
            stiffness: 1.0,
            gravity_power: 1.0,
            wind_scale: 1.0,
            ..default_config()
        };
        let pos = Vec3::new(1.0, 1.0, 1.0);
        let initial = Vec3::new(0.0, 2.0, 0.0);
        let wind = Vec3::new(1.0, 0.0, 0.0);
        let result = verlet_step(pos, pos, &config, initial, Vec3::ZERO, wind, 10.0);
        // dt should be clamped to MAX_DT=0.05, result must be finite
        assert!(result.x.is_finite());
        assert!(result.y.is_finite());
        assert!(result.z.is_finite());
        // Verify it equals what we'd get with dt=MAX_DT
        let expected = verlet_step(pos, pos, &config, initial, Vec3::ZERO, wind, MAX_DT);
        assert!((result - expected).length() < 1e-5);
    }

    #[test]
    fn zero_dt_no_movement() {
        let config = SpringConfig {
            drag_force: 1.0, // zero velocity
            stiffness: 2.0,
            gravity_power: 5.0,
            wind_scale: 3.0,
            ..default_config()
        };
        let pos = Vec3::new(1.0, 2.0, 3.0);
        let initial = Vec3::new(0.0, 5.0, 0.0);
        let wind = Vec3::new(1.0, 1.0, 1.0);
        let result = verlet_step(pos, pos, &config, initial, Vec3::ZERO, wind, 0.0);
        // dt=0 → stiffness, gravity, wind all scale by 0; drag=1 → velocity=0
        // result should equal current
        assert!((result - pos).length() < 1e-5);
    }

    #[test]
    fn nan_input_handled() {
        // When current == initial_world_tail, the stiffness direction is zero-length.
        // normalize_or_zero should return Vec3::ZERO, preventing NaN.
        let config = SpringConfig {
            drag_force: 1.0,
            stiffness: 2.0,
            gravity_power: 0.0,
            wind_scale: 0.0,
            ..default_config()
        };
        let pos = Vec3::new(1.0, 2.0, 3.0);
        let result = verlet_step(pos, pos, &config, pos, Vec3::ZERO, Vec3::ZERO, 0.016);
        // current == initial_world_tail → stiffness direction is zero → normalize_or_zero → Vec3::ZERO
        assert!(result.x.is_finite());
        assert!(result.y.is_finite());
        assert!(result.z.is_finite());
        // With drag=1 and no forces, result should equal current
        assert!((result - pos).length() < 1e-5);
    }
}
