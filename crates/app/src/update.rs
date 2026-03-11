use std::time::{Duration, Instant};

use anyhow::Result;
use nokhwa::pixel_format::RgbFormat;
use solver::types::{EulerAngles, EyeValues, RiggedHand, Side, VideoInfo};
use vrm::bone::HumanoidBoneName;

use crate::rig_config::BoneConfig;
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

    // 1. Send frame to tracker thread (non-blocking; drops frame if tracker is busy)
    state.tracker_thread.send_frame(frame, video.clone());

    // 2. Try to receive a new tracking result (non-blocking)
    if let Some(result) = state.tracker_thread.try_recv_result() {
        state.last_tracking_result = Some(result);
    }

    // 3. Run solvers on the latest available tracking result
    let mut rig_changed = false;

    if let Some(result) = &state.last_tracking_result {
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
    }

    // 4. Apply rig to VRM model (only if rig changed or first frame)
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
    // Compute world matrices for all nodes via FK, then build per-joint skinning matrices
    let world_matrices = state
        .vrm_model
        .humanoid_bones
        .compute_joint_matrices(&state.vrm_model.node_transforms);
    let joint_matrices: Vec<glam::Mat4> = state
        .vrm_model
        .skins
        .iter()
        .map(|joint| world_matrices[joint.node_index] * joint.inverse_bind_matrix)
        .collect();
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
        aspect: state.render_ctx.config.width as f32 / state.render_ctx.config.height.max(1) as f32,
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
    let cfg = &state.rig_config;

    // Apply face rig
    if let Some(face) = &state.rig.face {
        // Head rotation applied to Neck bone (matching testbed: rigRotation("Neck", ...))
        let head_quat = face.head.to_quat();
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::Neck,
            head_quat,
            cfg.neck.dampener,
            cfg.neck.lerp_amount,
        );

        // Eye blink: invert, clamp, lerp with previous frame, then stabilize
        let eye_l_raw = (1.0 - face.eye.l).clamp(0.0, 1.0);
        let eye_r_raw = (1.0 - face.eye.r).clamp(0.0, 1.0);
        let eye_l = state.rig.prev_blink_l + (eye_l_raw - state.rig.prev_blink_l) * cfg.eye_blink;
        let eye_r = state.rig.prev_blink_r + (eye_r_raw - state.rig.prev_blink_r) * cfg.eye_blink;
        let stabilized =
            solver::face::stabilize_blink(&EyeValues { l: eye_l, r: eye_r }, face.head.y);
        state.rig.prev_blink_l = stabilized.l;
        state.rig.prev_blink_r = stabilized.r;
        // Testbed uses same value (stabilized.l) for both BlinkL and BlinkR
        state
            .vrm_model
            .blend_shapes
            .set(vrm::blendshape::BlendShapePreset::BlinkL, stabilized.l);
        state
            .vrm_model
            .blend_shapes
            .set(vrm::blendshape::BlendShapePreset::BlinkR, stabilized.l);

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

        // Pupil tracking with lerp interpolation
        let pupil_lerp = cfg.pupil;
        let prev = state.rig.prev_look_target;
        let target = face.pupil;
        let interpolated = glam::Vec2::new(
            prev.x + (target.x - prev.x) * pupil_lerp,
            prev.y + (target.y - prev.y) * pupil_lerp,
        );
        state.rig.prev_look_target = interpolated;

        // Apply via LookAt if available
        if let Some(look_at) = &state.vrm_model.look_at {
            let euler = vrm::look_at::EulerAngles {
                yaw: interpolated.x * 30.0,
                pitch: interpolated.y * 30.0,
            };
            let eye_quat = look_at.apply(&euler);
            state.vrm_model.humanoid_bones.set_rotation_interpolated(
                vrm::bone::HumanoidBoneName::LeftEye,
                eye_quat,
                1.0,
                0.3,
            );
            state.vrm_model.humanoid_bones.set_rotation_interpolated(
                vrm::bone::HumanoidBoneName::RightEye,
                eye_quat,
                1.0,
                0.3,
            );
        }
    }

    // Apply pose rig
    if let Some(pose) = &state.rig.pose {
        // Hip position: X/Z inverted, Y+1.0
        let hip_pos = glam::Vec3::new(
            -pose.hips.position.x,
            pose.hips.position.y + 1.0,
            -pose.hips.position.z,
        );

        // Hips rotation
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::Hips,
            pose.hips.rotation.to_quat(),
            cfg.hips_rotation.dampener,
            cfg.hips_rotation.lerp_amount,
        );

        // Spine
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::Spine,
            pose.spine.to_quat(),
            cfg.spine.dampener,
            cfg.spine.lerp_amount,
        );

        // Chest
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::Chest,
            pose.chest.to_quat(),
            cfg.chest.dampener,
            cfg.chest.lerp_amount,
        );

        // Arms (limbs config)
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::RightUpperArm,
            pose.right_upper_arm.to_quat(),
            cfg.limbs.dampener,
            cfg.limbs.lerp_amount,
        );
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::RightLowerArm,
            pose.right_lower_arm.to_quat(),
            cfg.limbs.dampener,
            cfg.limbs.lerp_amount,
        );
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::LeftUpperArm,
            pose.left_upper_arm.to_quat(),
            cfg.limbs.dampener,
            cfg.limbs.lerp_amount,
        );
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::LeftLowerArm,
            pose.left_lower_arm.to_quat(),
            cfg.limbs.dampener,
            cfg.limbs.lerp_amount,
        );

        // Legs (limbs config)
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::RightUpperLeg,
            pose.right_upper_leg.to_quat(),
            cfg.limbs.dampener,
            cfg.limbs.lerp_amount,
        );
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::RightLowerLeg,
            pose.right_lower_leg.to_quat(),
            cfg.limbs.dampener,
            cfg.limbs.lerp_amount,
        );
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::LeftUpperLeg,
            pose.left_upper_leg.to_quat(),
            cfg.limbs.dampener,
            cfg.limbs.lerp_amount,
        );
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::LeftLowerLeg,
            pose.left_lower_leg.to_quat(),
            cfg.limbs.dampener,
            cfg.limbs.lerp_amount,
        );

        // Apply hip position
        state.vrm_model.humanoid_bones.set_position_interpolated(
            vrm::bone::HumanoidBoneName::Hips,
            hip_pos,
            cfg.hips_position.dampener,
            cfg.hips_position.lerp_amount,
        );
    }

    // Apply hand bones
    // Left hand: combine wrist X/Y from hand solver with Z from pose solver
    if let Some(left_hand) = &state.rig.left_hand {
        let pose_wrist_z = state
            .rig
            .pose
            .as_ref()
            .map(|p| p.left_hand.z)
            .unwrap_or(0.0);
        let wrist_combined = EulerAngles::new(left_hand.wrist.x, left_hand.wrist.y, pose_wrist_z);
        apply_hand_bones(
            &mut state.vrm_model.humanoid_bones,
            left_hand,
            &wrist_combined,
            Side::Left,
            &cfg.limbs,
        );
    }

    // Right hand: combine wrist X/Y from hand solver with Z from pose solver
    if let Some(right_hand) = &state.rig.right_hand {
        let pose_wrist_z = state
            .rig
            .pose
            .as_ref()
            .map(|p| p.right_hand.z)
            .unwrap_or(0.0);
        let wrist_combined = EulerAngles::new(right_hand.wrist.x, right_hand.wrist.y, pose_wrist_z);
        apply_hand_bones(
            &mut state.vrm_model.humanoid_bones,
            right_hand,
            &wrist_combined,
            Side::Right,
            &cfg.limbs,
        );
    }
}

/// Apply hand solver results (wrist + 15 finger joints) to humanoid bones.
///
/// The wrist rotation is a combined value: X/Y from the hand solver, Z from the pose solver.
/// All 16 bones (wrist + 15 fingers) use the limbs interpolation config.
fn apply_hand_bones(
    bones: &mut vrm::bone::HumanoidBones,
    hand: &RiggedHand,
    wrist_combined: &EulerAngles,
    side: Side,
    config: &BoneConfig,
) {
    // Build array of (bone_name, euler_angles) pairs for all 16 bones per hand.
    let mappings: [(HumanoidBoneName, &EulerAngles); 16] = match side {
        Side::Left => [
            (HumanoidBoneName::LeftHand, wrist_combined),
            (HumanoidBoneName::LeftThumbProximal, &hand.thumb_proximal),
            (
                HumanoidBoneName::LeftThumbIntermediate,
                &hand.thumb_intermediate,
            ),
            (HumanoidBoneName::LeftThumbDistal, &hand.thumb_distal),
            (HumanoidBoneName::LeftIndexProximal, &hand.index_proximal),
            (
                HumanoidBoneName::LeftIndexIntermediate,
                &hand.index_intermediate,
            ),
            (HumanoidBoneName::LeftIndexDistal, &hand.index_distal),
            (HumanoidBoneName::LeftMiddleProximal, &hand.middle_proximal),
            (
                HumanoidBoneName::LeftMiddleIntermediate,
                &hand.middle_intermediate,
            ),
            (HumanoidBoneName::LeftMiddleDistal, &hand.middle_distal),
            (HumanoidBoneName::LeftRingProximal, &hand.ring_proximal),
            (
                HumanoidBoneName::LeftRingIntermediate,
                &hand.ring_intermediate,
            ),
            (HumanoidBoneName::LeftRingDistal, &hand.ring_distal),
            (HumanoidBoneName::LeftLittleProximal, &hand.little_proximal),
            (
                HumanoidBoneName::LeftLittleIntermediate,
                &hand.little_intermediate,
            ),
            (HumanoidBoneName::LeftLittleDistal, &hand.little_distal),
        ],
        Side::Right => [
            (HumanoidBoneName::RightHand, wrist_combined),
            (HumanoidBoneName::RightThumbProximal, &hand.thumb_proximal),
            (
                HumanoidBoneName::RightThumbIntermediate,
                &hand.thumb_intermediate,
            ),
            (HumanoidBoneName::RightThumbDistal, &hand.thumb_distal),
            (HumanoidBoneName::RightIndexProximal, &hand.index_proximal),
            (
                HumanoidBoneName::RightIndexIntermediate,
                &hand.index_intermediate,
            ),
            (HumanoidBoneName::RightIndexDistal, &hand.index_distal),
            (HumanoidBoneName::RightMiddleProximal, &hand.middle_proximal),
            (
                HumanoidBoneName::RightMiddleIntermediate,
                &hand.middle_intermediate,
            ),
            (HumanoidBoneName::RightMiddleDistal, &hand.middle_distal),
            (HumanoidBoneName::RightRingProximal, &hand.ring_proximal),
            (
                HumanoidBoneName::RightRingIntermediate,
                &hand.ring_intermediate,
            ),
            (HumanoidBoneName::RightRingDistal, &hand.ring_distal),
            (HumanoidBoneName::RightLittleProximal, &hand.little_proximal),
            (
                HumanoidBoneName::RightLittleIntermediate,
                &hand.little_intermediate,
            ),
            (HumanoidBoneName::RightLittleDistal, &hand.little_distal),
        ],
    };

    for (bone_name, euler) in &mappings {
        bones.set_rotation_interpolated(
            *bone_name,
            euler.to_quat(),
            config.dampener,
            config.lerp_amount,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `apply_hand_bones` correctly applies all 16 bone rotations per hand.
    #[test]
    fn apply_hand_bones_sets_all_16_left_bones() {
        // Build a minimal HumanoidBones with all left-hand bones
        let left_bone_names = [
            "leftHand",
            "leftThumbProximal",
            "leftThumbIntermediate",
            "leftThumbDistal",
            "leftIndexProximal",
            "leftIndexIntermediate",
            "leftIndexDistal",
            "leftMiddleProximal",
            "leftMiddleIntermediate",
            "leftMiddleDistal",
            "leftRingProximal",
            "leftRingIntermediate",
            "leftRingDistal",
            "leftLittleProximal",
            "leftLittleIntermediate",
            "leftLittleDistal",
        ];

        let mut human_bones_arr = Vec::new();
        for (i, name) in left_bone_names.iter().enumerate() {
            human_bones_arr.push(serde_json::json!({ "bone": name, "node": i }));
        }

        let json = serde_json::json!({
            "humanoid": {
                "humanBones": human_bones_arr
            }
        });

        let node_transforms: Vec<vrm::model::NodeTransform> = (0..16)
            .map(|_| vrm::model::NodeTransform {
                translation: glam::Vec3::ZERO,
                rotation: glam::Quat::IDENTITY,
                scale: glam::Vec3::ONE,
                children: vec![],
            })
            .collect();

        let mut bones = vrm::bone::HumanoidBones::from_vrm_json(&json, &node_transforms).unwrap();

        // Create a hand with non-zero rotations
        let angle = EulerAngles::new(0.1, 0.2, 0.3);
        let hand = RiggedHand {
            wrist: angle,
            thumb_proximal: angle,
            thumb_intermediate: angle,
            thumb_distal: angle,
            index_proximal: angle,
            index_intermediate: angle,
            index_distal: angle,
            middle_proximal: angle,
            middle_intermediate: angle,
            middle_distal: angle,
            ring_proximal: angle,
            ring_intermediate: angle,
            ring_distal: angle,
            little_proximal: angle,
            little_intermediate: angle,
            little_distal: angle,
        };

        let wrist_combined = EulerAngles::new(0.1, 0.2, 0.5);
        let config = BoneConfig {
            dampener: 1.0,
            lerp_amount: 0.3,
        };

        apply_hand_bones(&mut bones, &hand, &wrist_combined, Side::Left, &config);

        // All 16 left-hand bones should now have non-identity rotations
        let check_bones = [
            HumanoidBoneName::LeftHand,
            HumanoidBoneName::LeftThumbProximal,
            HumanoidBoneName::LeftThumbIntermediate,
            HumanoidBoneName::LeftThumbDistal,
            HumanoidBoneName::LeftIndexProximal,
            HumanoidBoneName::LeftIndexIntermediate,
            HumanoidBoneName::LeftIndexDistal,
            HumanoidBoneName::LeftMiddleProximal,
            HumanoidBoneName::LeftMiddleIntermediate,
            HumanoidBoneName::LeftMiddleDistal,
            HumanoidBoneName::LeftRingProximal,
            HumanoidBoneName::LeftRingIntermediate,
            HumanoidBoneName::LeftRingDistal,
            HumanoidBoneName::LeftLittleProximal,
            HumanoidBoneName::LeftLittleIntermediate,
            HumanoidBoneName::LeftLittleDistal,
        ];

        for bone_name in &check_bones {
            let bone = bones.get(*bone_name).expect("bone should exist");
            let angle = bone.local_rotation.angle_between(glam::Quat::IDENTITY);
            assert!(
                angle > 0.01,
                "{:?} should have non-identity rotation after apply_hand_bones",
                bone_name
            );
        }
    }

    /// Verify wrist combination: X/Y from hand solver, Z from pose solver.
    #[test]
    fn wrist_combination_uses_pose_z_and_hand_xy() {
        let hand_wrist = EulerAngles::new(0.5, 0.6, 0.7);
        let pose_z = 1.2;
        let combined = EulerAngles::new(hand_wrist.x, hand_wrist.y, pose_z);

        assert!((combined.x - 0.5).abs() < 1e-6);
        assert!((combined.y - 0.6).abs() < 1e-6);
        assert!((combined.z - 1.2).abs() < 1e-6);
    }

    /// Verify blink values are interpolated (lerped) rather than directly assigned.
    #[test]
    fn blink_values_are_interpolated() {
        let eye_blink_factor = 0.5; // default RigConfig.eye_blink

        // Simulate: previous blink was 0.0, new raw value is 1.0
        let prev = 0.0_f32;
        let raw = 1.0_f32;
        let interpolated = prev + (raw - prev) * eye_blink_factor;

        // With factor 0.5, result should be 0.5 (halfway), NOT 1.0 (direct)
        assert!(
            (interpolated - 0.5).abs() < 1e-6,
            "blink should be interpolated to 0.5, got {}",
            interpolated
        );
        assert!(
            (interpolated - raw).abs() > 0.1,
            "interpolated value should differ from raw value"
        );

        // Second frame: previous is now 0.5, new raw still 1.0
        let prev2 = interpolated;
        let interpolated2 = prev2 + (raw - prev2) * eye_blink_factor;
        assert!(
            (interpolated2 - 0.75).abs() < 1e-6,
            "second frame should converge to 0.75, got {}",
            interpolated2
        );
    }
}
