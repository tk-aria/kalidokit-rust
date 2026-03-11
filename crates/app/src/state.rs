use std::time::Instant;

use renderer::context::RenderContext;
use renderer::scene::Scene;
use solver::types::{RiggedFace, RiggedHand, RiggedPose};
use tracker::HolisticResult;
use vrm::model::VrmModel;

use crate::rig_config::RigConfig;
use crate::tracker_thread::TrackerThread;

/// All application resources: renderer, tracker, solver results, VRM model.
pub struct AppState {
    pub render_ctx: RenderContext,
    pub scene: Scene,
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
