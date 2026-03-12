/// Direction preset for positioning a light relative to the avatar center.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LightPreset {
    Front,
    Back,
    Left,
    Right,
    FrontLeft,
    FrontRight,
    BackLeft,
    BackRight,
    Top,
    /// Custom xyz direction (not normalized — will be normalized in shader).
    Custom,
}

impl LightPreset {
    /// Convert preset to a direction vector (from avatar toward light).
    /// Y component adds slight elevation for all side presets.
    pub fn to_direction(self) -> [f32; 3] {
        match self {
            Self::Front => [0.0, 0.3, 1.0],
            Self::Back => [0.0, 0.3, -1.0],
            Self::Left => [-1.0, 0.3, 0.0],
            Self::Right => [1.0, 0.3, 0.0],
            Self::FrontLeft => [-0.7, 0.3, 0.7],
            Self::FrontRight => [0.7, 0.3, 0.7],
            Self::BackLeft => [-0.7, 0.3, -0.7],
            Self::BackRight => [0.7, 0.3, -0.7],
            Self::Top => [0.0, 1.0, 0.0],
            Self::Custom => [0.0, 1.0, 0.0], // fallback; caller sets direction manually
        }
    }

    /// All named presets (excluding Custom) for cycling.
    pub const ALL_NAMED: &[LightPreset] = &[
        Self::Front,
        Self::FrontRight,
        Self::Right,
        Self::BackRight,
        Self::Back,
        Self::BackLeft,
        Self::Left,
        Self::FrontLeft,
        Self::Top,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Front => "Front",
            Self::Back => "Back",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::FrontLeft => "FrontLeft",
            Self::FrontRight => "FrontRight",
            Self::BackLeft => "BackLeft",
            Self::BackRight => "BackRight",
            Self::Top => "Top",
            Self::Custom => "Custom",
        }
    }
}

/// A single stage light: direction, color, and intensity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StageLight {
    pub preset: LightPreset,
    /// Direction xyz (overridden by preset unless Custom).
    pub direction: [f32; 3],
    /// RGB color [0..1].
    pub color: [f32; 3],
    /// Intensity multiplier.
    pub intensity: f32,
}

impl StageLight {
    /// Get the effective direction (from preset or custom).
    pub fn effective_direction(&self) -> [f32; 3] {
        if self.preset == LightPreset::Custom {
            self.direction
        } else {
            self.preset.to_direction()
        }
    }

    /// Cycle to next preset position.
    pub fn next_preset(&mut self) {
        let presets = LightPreset::ALL_NAMED;
        let idx = presets.iter().position(|p| *p == self.preset).unwrap_or(0);
        self.preset = presets[(idx + 1) % presets.len()];
        self.direction = self.preset.to_direction();
    }
}

/// Shading mode for the fragment shader.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ShadingMode {
    /// Virtual live concert-style toon (3-point stage lighting, cel-shade, rim, specular).
    VirtualLive,
    /// Classic MToon (single directional light, 2-step toon, simple rim).
    Classic,
}

impl ShadingMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::VirtualLive => "VirtualLive",
            Self::Classic => "Classic",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::VirtualLive => Self::Classic,
            Self::Classic => Self::VirtualLive,
        }
    }

    fn to_f32(self) -> f32 {
        match self {
            Self::VirtualLive => 0.0,
            Self::Classic => 1.0,
        }
    }
}

/// 3-light stage lighting configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StageLighting {
    pub key: StageLight,
    pub fill: StageLight,
    pub back: StageLight,
    #[serde(default)]
    pub shading_mode: ShadingMode,
}

impl Default for ShadingMode {
    fn default() -> Self {
        Self::VirtualLive
    }
}

impl Default for StageLighting {
    fn default() -> Self {
        Self {
            key: StageLight {
                preset: LightPreset::FrontRight,
                direction: [0.8, 1.2, 0.6],
                color: [1.0, 0.95, 0.88],
                intensity: 1.4,
            },
            fill: StageLight {
                preset: LightPreset::FrontLeft,
                direction: [-0.7, 0.4, 0.3],
                color: [0.5, 0.55, 0.95],
                intensity: 0.5,
            },
            back: StageLight {
                preset: LightPreset::Back,
                direction: [0.0, 0.3, -1.0],
                color: [0.9, 0.5, 0.9],
                intensity: 0.6,
            },
            shading_mode: ShadingMode::default(),
        }
    }
}

/// GPU-side lights uniform. Matches the shader's `LightsUniform` struct.
///
/// Each light: direction (xyz) + intensity (w), color (rgb) + padding (w).
/// Total: 3 lights * 2 vec4 = 96 bytes.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightsUniform {
    pub light0_dir_intensity: [f32; 4],
    pub light0_color: [f32; 4],
    pub light1_dir_intensity: [f32; 4],
    pub light1_color: [f32; 4],
    pub light2_dir_intensity: [f32; 4],
    pub light2_color: [f32; 4],
}

impl StageLighting {
    pub fn to_uniform(&self) -> LightsUniform {
        let pack = |light: &StageLight| -> ([f32; 4], [f32; 4]) {
            let dir = light.effective_direction();
            (
                [dir[0], dir[1], dir[2], light.intensity],
                [light.color[0], light.color[1], light.color[2], 0.0],
            )
        };
        let (k_dir, k_col) = pack(&self.key);
        let (f_dir, f_col) = pack(&self.fill);
        let (b_dir, mut b_col) = pack(&self.back);
        // Encode shading mode in the w component of back light color (padding)
        b_col[3] = self.shading_mode.to_f32();
        LightsUniform {
            light0_dir_intensity: k_dir,
            light0_color: k_col,
            light1_dir_intensity: f_dir,
            light1_color: f_col,
            light2_dir_intensity: b_dir,
            light2_color: b_col,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_lighting_values() {
        let l = StageLighting::default();
        assert!((l.key.intensity - 1.4).abs() < 1e-6);
        assert!((l.fill.intensity - 0.5).abs() < 1e-6);
        assert!((l.back.intensity - 0.6).abs() < 1e-6);
    }

    #[test]
    fn uniform_is_pod() {
        let l = StageLighting::default();
        let u = l.to_uniform();
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 96); // 6 * vec4(16) = 96
    }

    #[test]
    fn preset_cycle() {
        let mut light = StageLight {
            preset: LightPreset::Front,
            direction: [0.0, 0.3, 1.0],
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
        };
        light.next_preset();
        assert_eq!(light.preset, LightPreset::FrontRight);
        // Cycle all the way around
        for _ in 0..8 {
            light.next_preset();
        }
        assert_eq!(light.preset, LightPreset::Front);
    }

    #[test]
    fn effective_direction_uses_preset() {
        let light = StageLight {
            preset: LightPreset::Back,
            direction: [99.0, 99.0, 99.0], // should be ignored
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
        };
        let dir = light.effective_direction();
        assert!((dir[2] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn effective_direction_custom() {
        let light = StageLight {
            preset: LightPreset::Custom,
            direction: [0.5, 0.5, 0.5],
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
        };
        let dir = light.effective_direction();
        assert!((dir[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn roundtrip_serialize() {
        let l = StageLighting::default();
        let yaml = serde_yaml::to_string(&l).unwrap();
        let loaded: StageLighting = serde_yaml::from_str(&yaml).unwrap();
        assert!((loaded.key.intensity - l.key.intensity).abs() < 1e-6);
    }
}
