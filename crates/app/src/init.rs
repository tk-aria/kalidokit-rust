use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use renderer::context::RenderContext;
use renderer::scene::{MeshMaterialInput, Scene};
use renderer::vertex::Vertex;
use tracker::holistic::HolisticTracker;

use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType};

use crate::tracker_thread::TrackerThread;
use winit::window::Window;

use crate::rig_config::RigConfig;
use crate::state::{AppState, RigState};

/// Default paths for ONNX models and VRM avatar.
const DEFAULT_VRM_PATH: &str = "assets/models/default_avatar.vrm";
const FACE_MODEL_PATH: &str = "assets/models/face_landmark.onnx";
const POSE_MODEL_PATH: &str = "assets/models/pose_landmark.onnx";
const HAND_MODEL_PATH: &str = "assets/models/hand_landmark.onnx";

/// Check that all required model files exist and return a helpful error if not.
fn check_model_files() -> Result<()> {
    let required = [
        (DEFAULT_VRM_PATH, "VRM avatar"),
        (FACE_MODEL_PATH, "Face landmark ONNX model"),
        (POSE_MODEL_PATH, "Pose landmark ONNX model"),
        (HAND_MODEL_PATH, "Hand landmark ONNX model"),
    ];

    let missing: Vec<_> = required
        .iter()
        .filter(|(path, _)| !Path::new(path).exists())
        .collect();

    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|(path, desc)| format!("  - {path} ({desc})"))
            .collect::<Vec<_>>()
            .join("\n");

        anyhow::bail!(
            "Required model files not found:\n{list}\n\n\
             To download them, run:\n  \
             sh scripts/setup.sh download-models\n\n\
             Or download manually — see README.md for details."
        );
    }
    Ok(())
}

/// Initialize all application resources.
///
/// 1. wgpu rendering context
/// 2. VRM model loading
/// 3. GPU scene creation
/// 4. ML tracker initialization
pub async fn init_all(window: Arc<Window>) -> Result<AppState> {
    // 0. Verify model files exist
    check_model_files()?;

    // 1. wgpu initialization
    let render_ctx = RenderContext::new(window).await?;

    // 2. Load VRM model
    let vrm_model = vrm::loader::load(DEFAULT_VRM_PATH)
        .context("Failed to load VRM avatar. Run: sh scripts/setup.sh download-models")?;

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

    // Build per-mesh material inputs from VRM materials
    let mesh_materials: Vec<MeshMaterialInput> = vrm_model
        .meshes
        .iter()
        .map(
            |mesh| match mesh.material_index.and_then(|i| vrm_model.materials.get(i)) {
                Some(mat) => MeshMaterialInput {
                    base_color: mat.base_color,
                    shade_color: mat.shade_color,
                    rim_color: mat.rim_color,
                    shade_shift: mat.shade_shift,
                    shade_toony: mat.shade_toony,
                    rim_power: mat.rim_power,
                    rim_lift: mat.rim_lift,
                    base_color_texture: mat.base_color_texture.clone(),
                },
                None => MeshMaterialInput::default(),
            },
        )
        .collect();

    let scene = Scene::new(
        &render_ctx.device,
        &render_ctx.queue,
        &render_ctx.config,
        &vertices_list,
        &mesh_materials,
        max_joints,
        num_morph_targets,
    );

    // 4. Initialize ML tracker on a background thread
    let tracker = HolisticTracker::new(FACE_MODEL_PATH, POSE_MODEL_PATH, HAND_MODEL_PATH)
        .context("Failed to initialize ML tracker. Run: sh scripts/setup.sh download-models")?;
    let tracker_thread = TrackerThread::new(tracker);

    // 5. Initialize webcam via nokhwa
    let camera = match init_camera() {
        Ok(cam) => Some(cam),
        Err(e) => {
            log::warn!("Failed to initialize webcam: {e}. Falling back to dummy frames.");
            None
        }
    };

    Ok(AppState {
        render_ctx,
        scene,
        vrm_model,
        tracker_thread,
        camera,
        rig: RigState::default(),
        rig_config: RigConfig::default(),
        last_frame_time: Instant::now(),
        rig_dirty: true,
        last_tracking_result: None,
    })
}

/// Try to initialize the default webcam (index 0) at 640x480.
fn init_camera() -> Result<nokhwa::Camera> {
    let index = CameraIndex::Index(0);
    let format = CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30);
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(format));
    let mut camera = nokhwa::Camera::new(index, requested).context("Failed to create camera")?;
    camera
        .open_stream()
        .context("Failed to open camera stream")?;
    log::info!("Webcam initialized: {:?}", camera.camera_format());
    Ok(camera)
}
