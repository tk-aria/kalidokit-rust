use std::time::Instant;

use renderer::context::RenderContext;
use renderer::debug_overlay::DebugOverlay;
use renderer::light::StageLighting;
use renderer::scene::Scene;
use solver::types::{RiggedFace, RiggedHand, RiggedPose};
use tracker::HolisticResult;
use vrm::model::VrmModel;

use crate::auto_blink::{AutoBlink, BlinkMode};
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
