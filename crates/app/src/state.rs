use std::time::Instant;

use renderer::context::RenderContext;
use renderer::debug_overlay::DebugOverlay;
use renderer::light::{BackgroundConfig, StageLighting};
use renderer::scene::Scene;
use solver::types::{RiggedFace, RiggedHand, RiggedPose};
use tracker::HolisticResult;
use vrm::model::VrmModel;

use crate::auto_blink::{AutoBlink, BlinkMode};
use crate::mascot::MascotState;
use crate::rig_config::RigConfig;
use crate::tracker_thread::TrackerThread;
use vrm::animation_player::AnimationPlayer;

/// All application resources: renderer, tracker, solver results, VRM model.
pub struct AppState {
    pub render_ctx: RenderContext,
    pub scene: Scene,
    pub debug_overlay: DebugOverlay,
    pub vrm_model: VrmModel,
    pub tracker_thread: TrackerThread,
    /// Webcam camera handle (None if camera initialization failed).
    pub camera: Option<nokhwa::Camera>,
    pub rig: RigState,
    pub rig_config: RigConfig,
    pub last_frame_time: Instant,
    pub rig_dirty: bool,
    /// Cached latest tracking result from the background tracker thread.
    pub last_tracking_result: Option<HolisticResult>,
    /// Camera distance from the look-at target (controlled by mouse wheel zoom).
    pub camera_distance: f32,
    /// Latest camera frame for debug overlay display.
    pub last_camera_frame: Option<image::DynamicImage>,
    /// Blink mode: Tracking (webcam) or Auto (periodic random blinks).
    pub blink_mode: BlinkMode,
    /// Auto blink controller (used when blink_mode == Auto).
    pub auto_blink: AutoBlink,
    /// Stage lighting configuration (3-point lights).
    pub stage_lighting: StageLighting,
    /// Virtual camera for streaming to video apps (None if not started).
    #[cfg(target_os = "macos")]
    pub vcam: Option<virtual_camera::MacOsVirtualCamera>,
    /// Whether virtual camera streaming is enabled.
    pub vcam_enabled: bool,
    /// Last time a frame was sent to the virtual camera (for 30fps throttle).
    pub vcam_last_send: Instant,
    /// Idle animation player for blending with tracking.
    pub idle_animation: Option<AnimationPlayer>,
    /// Whether tracking is enabled (T key to toggle).
    pub tracking_enabled: bool,
    /// Animation path from config (preserved for save).
    pub animation_path: Option<String>,
    /// Background config (preserved for save).
    pub background: BackgroundConfig,
    /// Video background decode session (None if not using video background).
    pub video_session: Option<Box<dyn video_decoder::VideoSession>>,
    /// FPS counter: frames rendered since last reset.
    pub fps_counter: u32,
    /// FPS counter: video frames decoded since last reset.
    pub fps_decode_counter: u32,
    /// FPS counter: timestamp of last reset.
    pub fps_timer: Instant,
    /// Desktop mascot overlay state.
    pub mascot: MascotState,
    /// Last known cursor position (physical pixels), used for mascot drag.
    pub last_cursor_pos: winit::dpi::PhysicalPosition<f64>,
    /// Cached alpha map from rendered frame for mascot pixel-alpha hit-testing.
    /// One byte per pixel (alpha channel only), dimensions match `mascot_alpha_width` x `mascot_alpha_height`.
    pub mascot_alpha_map: Vec<u8>,
    /// Width of the cached mascot alpha map in pixels.
    pub mascot_alpha_width: u32,
    /// Height of the cached mascot alpha map in pixels.
    pub mascot_alpha_height: u32,
    /// ImGui debug/settings UI renderer (None if initialization failed).
    pub imgui: Option<imgui_renderer::ImGuiRenderer>,
    /// Whether ImGui UI is visible (toggled by F1).
    pub show_imgui: bool,
    /// Whether the window is in fullscreen mode.
    pub fullscreen: bool,
    /// Whether the debug overlay (camera preview + landmarks) is shown.
    pub show_debug_overlay: bool,
    /// Pending mascot mode toggle (deferred to after surface present).
    pub pending_mascot_toggle: bool,
    /// Model offset for dragging the avatar within the window (non-mascot mode).
    pub model_offset: [f32; 2],
    /// Whether the user is dragging the avatar (non-mascot mode).
    pub dragging_model: bool,
    /// Last cursor position when model drag started.
    pub drag_prev_pos: [f64; 2],
}

/// Current rig solver results (face/pose/hand).
#[derive(Default)]
pub struct RigState {
    pub face: Option<RiggedFace>,
    pub pose: Option<RiggedPose>,
    pub left_hand: Option<RiggedHand>,
    pub right_hand: Option<RiggedHand>,
    /// Previous interpolated look target for pupil lerp smoothing.
    pub prev_look_target: glam::Vec2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rig_state_default_all_none() {
        let rig = RigState::default();
        assert!(rig.face.is_none());
        assert!(rig.pose.is_none());
        assert!(rig.left_hand.is_none());
        assert!(rig.right_hand.is_none());
    }
}
