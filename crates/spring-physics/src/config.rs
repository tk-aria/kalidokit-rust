use glam::Vec3;

/// Configuration for a single spring bone chain.
#[derive(Debug, Clone)]
pub struct SpringConfig {
    /// Spring stiffness (clamped to 0.0..4.0).
    pub stiffness: f32,
    /// Gravity strength (clamped to 0.0..10.0).
    pub gravity_power: f32,
    /// Normalized gravity direction.
    pub gravity_dir: Vec3,
    /// Drag coefficient (clamped to 0.0..1.0).
    pub drag_force: f32,
    /// Collision sphere radius (clamped to 0.0..1.0).
    pub hit_radius: f32,
    /// Wind influence scale (clamped to 0.0..10.0).
    pub wind_scale: f32,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            stiffness: 1.0,
            gravity_power: 0.0,
            gravity_dir: Vec3::new(0.0, -1.0, 0.0),
            drag_force: 0.4,
            hit_radius: 0.02,
            wind_scale: 0.0,
        }
    }
}

impl SpringConfig {
    /// Clamp all fields to their valid ranges.
    pub fn validate(&mut self) {
        self.stiffness = self.stiffness.clamp(0.0, 4.0);
        // Minimum drag to prevent undamped oscillation
        self.drag_force = self.drag_force.clamp(0.15, 1.0);
        self.gravity_power = self.gravity_power.clamp(0.0, 10.0);
        self.hit_radius = self.hit_radius.clamp(0.0, 1.0);
        self.wind_scale = self.wind_scale.clamp(0.0, 10.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_are_valid() {
        let mut cfg = SpringConfig::default();
        let before = cfg.clone();
        cfg.validate();
        assert_eq!(cfg.stiffness, before.stiffness);
        assert_eq!(cfg.gravity_power, before.gravity_power);
        assert_eq!(cfg.drag_force, before.drag_force);
        assert_eq!(cfg.hit_radius, before.hit_radius);
        assert_eq!(cfg.wind_scale, before.wind_scale);
    }

    #[test]
    fn validate_clamps_out_of_range() {
        let mut cfg = SpringConfig {
            stiffness: 5.0,
            gravity_power: 20.0,
            drag_force: 2.0,
            hit_radius: 5.0,
            wind_scale: 100.0,
            ..Default::default()
        };
        cfg.validate();
        assert_eq!(cfg.stiffness, 4.0);
        assert_eq!(cfg.gravity_power, 10.0);
        assert_eq!(cfg.drag_force, 1.0);
        assert_eq!(cfg.hit_radius, 1.0);
        assert_eq!(cfg.wind_scale, 10.0);
    }

    #[test]
    fn negative_stiffness_clamped_to_zero() {
        let mut cfg = SpringConfig {
            stiffness: -3.0,
            ..Default::default()
        };
        cfg.validate();
        assert_eq!(cfg.stiffness, 0.0);
    }

    #[test]
    fn all_negative_values_clamped_to_zero() {
        let mut cfg = SpringConfig {
            stiffness: -1.0,
            gravity_power: -5.0,
            drag_force: -0.5,
            hit_radius: -0.1,
            wind_scale: -2.0,
            ..Default::default()
        };
        cfg.validate();
        assert_eq!(cfg.stiffness, 0.0);
        assert_eq!(cfg.gravity_power, 0.0);
        assert_eq!(cfg.drag_force, 0.15); // minimum drag to prevent undamped oscillation
        assert_eq!(cfg.hit_radius, 0.0);
        assert_eq!(cfg.wind_scale, 0.0);
    }

    #[test]
    fn drag_force_above_one_clamped() {
        let mut cfg = SpringConfig {
            drag_force: 1.5,
            ..Default::default()
        };
        cfg.validate();
        assert_eq!(cfg.drag_force, 1.0);
    }
}
