use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use renderer::context::RenderContext;
use renderer::debug_overlay::DebugOverlay;
use renderer::scene::{MeshMaterialInput, Scene};
use renderer::vertex::Vertex;
use tracker::holistic::HolisticTracker;

use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType};

use crate::auto_blink::AutoBlink;
use crate::tracker_thread::TrackerThread;
use crate::user_prefs::UserPrefs;
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

    // Log VRM model stats for debugging skinning pipeline
    {
        let total_verts: usize = vrm_model.meshes.iter().map(|m| m.vertices.len()).sum();
        let skinned_verts: usize = vrm_model
            .meshes
            .iter()
            .flat_map(|m| &m.vertices)
            .filter(|v| v.joint_weights.iter().any(|w| *w > 0.0))
            .count();
        pipeline_logger::bone(log::Level::Info, "VRM model loaded")
            .field("meshes", vrm_model.meshes.len())
            .field("total_verts", total_verts)
            .field("skinned_verts", skinned_verts)
            .field("skin_joints", vrm_model.skins.len())
            .field("materials", vrm_model.materials.len())
            .field("node_transforms", vrm_model.node_transforms.len())
            .emit();
    }

    // 3. Create GPU scene from VRM model meshes
    let vertices_list: Vec<(&[Vertex], &[u32])> = vrm_model
        .meshes
        .iter()
        .map(|m| (m.vertices.as_slice(), m.indices.as_slice()))
        .collect();

    let max_joints = vrm_model.skins.len().max(1);

    // Extract per-mesh morph target position deltas for GPU upload
    let mesh_morph_targets: Vec<Vec<Vec<[f32; 3]>>> = vrm_model
        .meshes
        .iter()
        .map(|m| {
            m.morph_targets
                .iter()
                .map(|t| t.position_deltas.clone())
                .collect()
        })
        .collect();

    {
        let total_targets: usize = mesh_morph_targets.iter().map(|t| t.len()).sum();
        pipeline_logger::bone(log::Level::Info, "morph targets extracted")
            .field("meshes_with_targets", mesh_morph_targets.iter().filter(|t| !t.is_empty()).count())
            .field("total_targets", total_targets)
            .emit();

        // Log per-mesh morph target info
        for (i, targets) in mesh_morph_targets.iter().enumerate() {
            if !targets.is_empty() {
                let verts = vrm_model.meshes[i].vertices.len();
                let max_delta: f32 = targets.iter()
                    .flat_map(|t| t.iter())
                    .flat_map(|d| d.iter())
                    .map(|v| v.abs())
                    .fold(0.0f32, f32::max);
                pipeline_logger::bone(log::Level::Info, "mesh morph targets")
                    .field("mesh_index", i)
                    .field("num_targets", targets.len())
                    .field("num_vertices", verts)
                    .field("max_delta", format!("{:.6}", max_delta))
                    .emit();
            }
        }

        // Log blend shape bindings
        let bindings = vrm_model.blend_shapes.debug_bindings();
        for (preset, binds) in &bindings {
            pipeline_logger::bone(log::Level::Info, "blend shape binding")
                .field("preset", preset.as_str())
                .field("binds", format!("{:?}", binds))
                .emit();
        }
    }

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

    let prefs = UserPrefs::load();
    log::info!("User prefs loaded: {:?}", prefs);

    let scene = Scene::new(
        &render_ctx.device,
        &render_ctx.queue,
        &render_ctx.config,
        &vertices_list,
        &mesh_materials,
        &mesh_morph_targets,
        max_joints,
        &prefs.stage_lighting,
    );

    // 4. Initialize ML tracker on a background thread (face-only mode for debugging)
    let tracker = HolisticTracker::new(FACE_MODEL_PATH, POSE_MODEL_PATH, HAND_MODEL_PATH)
        .context("Failed to initialize ML tracker. Run: sh scripts/setup.sh download-models")?;
    let tracker_thread = TrackerThread::new_with_mode(tracker, true);

    // 5. Initialize webcam via nokhwa
    let camera = match init_camera() {
        Ok(cam) => {
            pipeline_logger::camera(log::Level::Info, "webcam initialized")
                .field("format", format!("{:?}", cam.camera_format()))
                .emit();
            Some(cam)
        }
        Err(e) => {
            pipeline_logger::camera(log::Level::Warn, "webcam init failed, using dummy frames")
                .field("error", format!("{e}"))
                .emit();
            None
        }
    };

    // 6. Initialize debug overlay
    let debug_overlay = DebugOverlay::new(
        &render_ctx.device,
        &render_ctx.queue,
        render_ctx.config.format,
    );

    Ok(AppState {
        render_ctx,
        scene,
        debug_overlay,
        vrm_model,
        tracker_thread,
        camera,
        rig: RigState::default(),
        rig_config: RigConfig::default(),
        last_frame_time: Instant::now(),
        rig_dirty: true,
        last_tracking_result: None,
        camera_distance: prefs.camera_distance,
        last_camera_frame: None,
        blink_mode: prefs.blink_mode,
        auto_blink: AutoBlink::new(),
        stage_lighting: prefs.stage_lighting.clone(),
        #[cfg(target_os = "macos")]
        vcam: None,
        vcam_enabled: false,
    })
}

/// Try to initialize the default webcam (index 0) at 640x480.
fn init_camera() -> Result<nokhwa::Camera> {
    // On macOS, request camera permission via AVFoundation before creating the camera.
    // This blocks until the user grants/denies permission.
    let (tx, rx) = std::sync::mpsc::channel();
    nokhwa::nokhwa_initialize(move |granted| {
        let _ = tx.send(granted);
    });
    let granted = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .unwrap_or(false);
    if !granted {
        anyhow::bail!("Camera permission denied by user");
    }

    let index = CameraIndex::Index(0);
    let format = CameraFormat::new_from(640, 480, FrameFormat::YUYV, 30);
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(format));
    let mut camera = nokhwa::Camera::new(index, requested).context("Failed to create camera")?;
    camera
        .open_stream()
        .context("Failed to open camera stream")?;
    log::info!("Webcam initialized: {:?}", camera.camera_format());
    Ok(camera)
}
