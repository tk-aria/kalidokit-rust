use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use renderer::context::RenderContext;
use renderer::scene::Scene;
use renderer::vertex::Vertex;
use tracker::holistic::HolisticTracker;
use winit::window::Window;

use crate::state::{AppState, RigState};

/// Default paths for ONNX models and VRM avatar.
const DEFAULT_VRM_PATH: &str = "assets/models/default_avatar.vrm";
const FACE_MODEL_PATH: &str = "assets/models/face_landmark.onnx";
const POSE_MODEL_PATH: &str = "assets/models/pose_landmark.onnx";
const HAND_MODEL_PATH: &str = "assets/models/hand_landmark.onnx";

/// Initialize all application resources.
///
/// 1. wgpu rendering context
/// 2. VRM model loading
/// 3. GPU scene creation
/// 4. ML tracker initialization
pub async fn init_all(window: Arc<Window>) -> Result<AppState> {
    // 1. wgpu initialization
    let render_ctx = RenderContext::new(window).await?;

    // 2. Load VRM model
    let vrm_model = vrm::loader::load(DEFAULT_VRM_PATH)?;

    // 3. Create GPU scene from VRM model meshes
    let vertices_list: Vec<(&[Vertex], &[u32])> = vrm_model
        .meshes
        .iter()
        .map(|m| (m.vertices.as_slice(), m.indices.as_slice()))
        .collect();

    let max_joints = vrm_model.skins.len().max(1);
    let num_morph_targets = vrm_model
        .meshes
        .iter()
        .flat_map(|m| &m.morph_targets)
        .count()
        .max(1);

    let scene = Scene::new(
        &render_ctx.device,
        &render_ctx.config,
        &vertices_list,
        max_joints,
        num_morph_targets,
    );

    // 4. Initialize ML tracker
    let tracker = HolisticTracker::new(FACE_MODEL_PATH, POSE_MODEL_PATH, HAND_MODEL_PATH)?;

    Ok(AppState {
        render_ctx,
        scene,
        vrm_model,
        tracker,
        rig: RigState::default(),
        last_frame_time: Instant::now(),
        rig_dirty: true,
    })
}
