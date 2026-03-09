use std::time::{Duration, Instant};

use anyhow::Result;
use nokhwa::pixel_format::RgbFormat;
use solver::types::{Side, VideoInfo};

use crate::state::AppState;

/// Target frame duration: ~16ms for 60fps.
const TARGET_FRAME_DURATION: Duration = Duration::from_millis(16);

/// Update one frame: track → solve → apply → render.
/// Returns early if less than 16ms has elapsed (frame rate limiting).
pub fn update_frame(state: &mut AppState) -> Result<()> {
    // Frame rate control: skip if less than 16ms elapsed
    let now = Instant::now();
    let elapsed = now.duration_since(state.last_frame_time);
    if elapsed < TARGET_FRAME_DURATION {
        return Ok(());
    }
    state.last_frame_time = now;

    // Capture frame from webcam, falling back to dummy black image
    let (frame, video) = capture_frame(&mut state.camera);

    // 1. Run tracker on frame
    let result = state.tracker.detect(&frame)?;

    // 2. Run solvers on detected landmarks
    let mut rig_changed = false;

    if let Some(face_lm) = &result.face_landmarks {
        let face = solver::face::solve(face_lm, &video);
        state.rig.face = Some(face);
        rig_changed = true;
    }

    if let Some(pose_3d) = &result.pose_landmarks_3d {
        let pose_2d = result.pose_landmarks_2d.as_deref().unwrap_or(&[]);
        let pose_2d_vec: Vec<glam::Vec2> = if pose_2d.is_empty() {
            vec![glam::Vec2::ZERO; 33]
        } else {
            pose_2d.to_vec()
        };
        let pose = solver::pose::solve(pose_3d, &pose_2d_vec, &video);
        state.rig.pose = Some(pose);
        rig_changed = true;
    }

    if let Some(left_lm) = &result.left_hand_landmarks {
        let hand = solver::hand::solve(left_lm, Side::Left);
        state.rig.left_hand = Some(hand);
        rig_changed = true;
    }

    if let Some(right_lm) = &result.right_hand_landmarks {
        let hand = solver::hand::solve(right_lm, Side::Right);
        state.rig.right_hand = Some(hand);
        rig_changed = true;
    }

    // 3. Apply rig to VRM model (only if rig changed or first frame)
    if rig_changed || state.rig_dirty {
        apply_rig_to_model(state);
        state.rig_dirty = false;
    }

    // 3.5. Update spring bone physics
    let delta_time = elapsed.as_secs_f32();
    for group in &mut state.vrm_model.spring_bone_groups {
        group.update(delta_time, glam::Vec3::ZERO);
    }

    // 4. Update GPU buffers
    let joint_matrices = state
        .vrm_model
        .humanoid_bones
        .compute_joint_matrices(&state.vrm_model.node_transforms);
    let num_morph_targets = state
        .vrm_model
        .meshes
        .iter()
        .flat_map(|m| &m.morph_targets)
        .count()
        .max(1);
    let morph_weights = state
        .vrm_model
        .blend_shapes
        .get_all_weights(num_morph_targets);

    let camera = renderer::camera::Camera {
        aspect: state.render_ctx.config.width as f32
            / state.render_ctx.config.height.max(1) as f32,
        ..renderer::camera::Camera::default()
    };
    let camera_uniform = camera.to_uniform(glam::Mat4::IDENTITY);

    state.scene.prepare(
        &state.render_ctx.queue,
        &joint_matrices,
        &morph_weights,
        &camera_uniform,
    );

    // 5. Render
    state.scene.render(&state.render_ctx)?;

    Ok(())
}

/// Capture a frame from the webcam if available, otherwise return a dummy black image.
fn capture_frame(camera: &mut Option<nokhwa::Camera>) -> (image::DynamicImage, VideoInfo) {
    const FALLBACK_W: u32 = 640;
    const FALLBACK_H: u32 = 480;

    if let Some(cam) = camera.as_mut() {
        match cam.frame() {
            Ok(buffer) => {
                let res = buffer.resolution();
                let width = res.width_x;
                let height = res.height_y;
                match buffer.decode_image::<RgbFormat>() {
                    Ok(rgb_image) => {
                        let frame = image::DynamicImage::ImageRgb8(rgb_image);
                        let video = VideoInfo { width, height };
                        return (frame, video);
                    }
                    Err(e) => {
                        log::warn!("Failed to decode webcam frame: {e}");
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to capture webcam frame: {e}");
            }
        }
    }

    // Fallback: dummy black image
    let frame = image::DynamicImage::new_rgb8(FALLBACK_W, FALLBACK_H);
    let video = VideoInfo {
        width: FALLBACK_W,
        height: FALLBACK_H,
    };
    (frame, video)
}

/// Apply solver results to VRM model bones and blend shapes.
fn apply_rig_to_model(state: &mut AppState) {
    // Apply face rig
    if let Some(face) = &state.rig.face {
        // Head rotation
        let head_quat = face.head.to_quat();
        state
            .vrm_model
            .humanoid_bones
            .set_rotation(vrm::bone::HumanoidBoneName::Head, head_quat);

        // Eye blink (inverted: 1.0 - value)
        let eye_l = 1.0 - face.eye.l;
        let eye_r = 1.0 - face.eye.r;
        state
            .vrm_model
            .blend_shapes
            .set(vrm::blendshape::BlendShapePreset::BlinkL, eye_l);
        state
            .vrm_model
            .blend_shapes
            .set(vrm::blendshape::BlendShapePreset::BlinkR, eye_r);

        // Mouth shapes
        state
            .vrm_model
            .blend_shapes
            .set(vrm::blendshape::BlendShapePreset::A, face.mouth.a);
        state
            .vrm_model
            .blend_shapes
            .set(vrm::blendshape::BlendShapePreset::I, face.mouth.i);
        state
            .vrm_model
            .blend_shapes
            .set(vrm::blendshape::BlendShapePreset::U, face.mouth.u);
        state
            .vrm_model
            .blend_shapes
            .set(vrm::blendshape::BlendShapePreset::E, face.mouth.e);
        state
            .vrm_model
            .blend_shapes
            .set(vrm::blendshape::BlendShapePreset::O, face.mouth.o);
    }

    // Apply pose rig
    if let Some(pose) = &state.rig.pose {
        // Hip position: X/Z inverted, Y+1.0
        let hip_pos = glam::Vec3::new(
            -pose.hips.position.x,
            pose.hips.position.y + 1.0,
            -pose.hips.position.z,
        );
        state.vrm_model.humanoid_bones.set_rotation(
            vrm::bone::HumanoidBoneName::Hips,
            pose.hips.rotation.to_quat(),
        );

        state
            .vrm_model
            .humanoid_bones
            .set_rotation(vrm::bone::HumanoidBoneName::Spine, pose.spine.to_quat());
        state
            .vrm_model
            .humanoid_bones
            .set_rotation(vrm::bone::HumanoidBoneName::Chest, pose.chest.to_quat());
        state.vrm_model.humanoid_bones.set_rotation(
            vrm::bone::HumanoidBoneName::RightUpperArm,
            pose.right_upper_arm.to_quat(),
        );
        state.vrm_model.humanoid_bones.set_rotation(
            vrm::bone::HumanoidBoneName::RightLowerArm,
            pose.right_lower_arm.to_quat(),
        );
        state.vrm_model.humanoid_bones.set_rotation(
            vrm::bone::HumanoidBoneName::LeftUpperArm,
            pose.left_upper_arm.to_quat(),
        );
        state.vrm_model.humanoid_bones.set_rotation(
            vrm::bone::HumanoidBoneName::LeftLowerArm,
            pose.left_lower_arm.to_quat(),
        );

        // Legs
        state.vrm_model.humanoid_bones.set_rotation(
            vrm::bone::HumanoidBoneName::RightUpperLeg,
            pose.right_upper_leg.to_quat(),
        );
        state.vrm_model.humanoid_bones.set_rotation(
            vrm::bone::HumanoidBoneName::RightLowerLeg,
            pose.right_lower_leg.to_quat(),
        );
        state.vrm_model.humanoid_bones.set_rotation(
            vrm::bone::HumanoidBoneName::LeftUpperLeg,
            pose.left_upper_leg.to_quat(),
        );
        state.vrm_model.humanoid_bones.set_rotation(
            vrm::bone::HumanoidBoneName::LeftLowerLeg,
            pose.left_lower_leg.to_quat(),
        );

        // Store hip position for potential use
        let _ = hip_pos;
    }
}
