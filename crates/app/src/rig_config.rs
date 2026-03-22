/// Per-bone interpolation configuration.
#[derive(Debug, Clone, Copy)]
pub struct BoneConfig {
    pub dampener: f32,
    pub lerp_amount: f32,
}

/// Rig interpolation parameters matching the original KalidoKit implementation.
#[derive(Debug, Clone)]
pub struct RigConfig {
    pub neck: BoneConfig,
    pub hips_rotation: BoneConfig,
    pub hips_position: BoneConfig,
    pub chest: BoneConfig,
    pub spine: BoneConfig,
    pub limbs: BoneConfig,
    /// Separate config for hand/finger bones (heavier smoothing).
    pub fingers: BoneConfig,
    #[allow(dead_code)]
    pub eye_blink: f32,
    #[allow(dead_code)]
    pub mouth_shape: f32,
    pub pupil: f32,
}

impl Default for RigConfig {
    fn default() -> Self {
        Self {
            neck: BoneConfig {
                dampener: 0.7,
                lerp_amount: 0.3,
            },
            hips_rotation: BoneConfig {
                dampener: 0.7,
                lerp_amount: 0.3,
            },
            hips_position: BoneConfig {
                dampener: 1.0,
                lerp_amount: 0.07,
            },
            chest: BoneConfig {
                dampener: 0.25,
                lerp_amount: 0.3,
            },
            spine: BoneConfig {
                dampener: 0.45,
                lerp_amount: 0.3,
            },
            limbs: BoneConfig {
                dampener: 1.0,
                lerp_amount: 0.3,
            },
            fingers: BoneConfig {
                dampener: 1.0,
                lerp_amount: 0.15, // heavier smoothing for fingers to reduce jitter
            },
            eye_blink: 0.5,
            mouth_shape: 0.5,
            pupil: 0.4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_match_kalidokit() {
        let cfg = RigConfig::default();
        assert!((cfg.neck.dampener - 0.7).abs() < 1e-6);
        assert!((cfg.neck.lerp_amount - 0.3).abs() < 1e-6);
        assert!((cfg.hips_position.lerp_amount - 0.07).abs() < 1e-6);
        assert!((cfg.chest.dampener - 0.25).abs() < 1e-6);
        assert!((cfg.spine.dampener - 0.45).abs() < 1e-6);
        assert!((cfg.limbs.dampener - 1.0).abs() < 1e-6);
        assert!((cfg.eye_blink - 0.5).abs() < 1e-6);
        assert!((cfg.mouth_shape - 0.5).abs() < 1e-6);
        assert!((cfg.pupil - 0.4).abs() < 1e-6);
    }
}
