//! Avatar state definitions — shared between Rust app and Lua scripts.

/// Complete avatar state snapshot, readable/writable from Lua.
#[derive(Debug, Clone)]
pub struct AvatarState {
    pub info: InfoState,
    pub display: DisplayState,
    pub tracking: TrackingState,
    pub lighting: LightingState,
}

impl Default for AvatarState {
    fn default() -> Self {
        Self {
            info: InfoState::default(),
            display: DisplayState::default(),
            tracking: TrackingState::default(),
            lighting: LightingState::default(),
        }
    }
}

/// Read-only info snapshot (written by app, read by Lua).
#[derive(Debug, Clone, Default)]
pub struct InfoState {
    pub render_fps: u32,
    pub decode_fps: u32,
    pub frame_ms: f32,
    pub shading_mode: String,
    pub idle_anim_status: String,
    pub imgui_version: String,
}

/// Display-related state.
#[derive(Debug, Clone)]
pub struct DisplayState {
    pub mascot_enabled: bool,
    pub always_on_top: bool,
    pub fullscreen: bool,
    pub debug_overlay: bool,
    pub camera_distance: f32,
    pub model_offset: [f32; 2],
    pub bg_image_path: String,
    /// true = avatar renders on top of ImGui, false = ImGui on top (default).
    pub avatar_on_top: bool,
    /// Enable spring-physics simulation on the avatar (hair/cloth secondary motion).
    pub spring_physics_enabled: bool,
}

impl Default for DisplayState {
    fn default() -> Self {
        Self {
            mascot_enabled: false,
            always_on_top: false,
            fullscreen: false,
            debug_overlay: true,
            camera_distance: 3.0,
            model_offset: [0.0, 0.0],
            bg_image_path: String::new(),
            avatar_on_top: false,
            spring_physics_enabled: true,
        }
    }
}

/// Tracking-related state.
#[derive(Debug, Clone)]
pub struct TrackingState {
    pub tracking_enabled: bool,
    pub auto_blink: bool,
    pub idle_animation: bool,
    pub has_idle_animation: bool,
    pub vcam_enabled: bool,
    pub virtual_live_shading: bool,
    /// Per-feature tracking toggles
    pub face_tracking: bool,
    pub arm_tracking: bool,
    pub hand_tracking: bool,
}

impl Default for TrackingState {
    fn default() -> Self {
        Self {
            tracking_enabled: true,
            auto_blink: false,
            idle_animation: false,
            has_idle_animation: false,
            vcam_enabled: false,
            virtual_live_shading: true,
            face_tracking: true,
            arm_tracking: false,
            hand_tracking: false,
        }
    }
}

/// Per-light state.
#[derive(Debug, Clone)]
pub struct LightState {
    pub intensity: f32,
    pub color: [f32; 3],
    pub preset: String,
}

impl Default for LightState {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            color: [1.0, 1.0, 1.0],
            preset: String::new(),
        }
    }
}

/// 3-light stage lighting state.
#[derive(Debug, Clone, Default)]
pub struct LightingState {
    pub key: LightState,
    pub fill: LightState,
    pub back: LightState,
}
