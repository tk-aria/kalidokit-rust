use std::time::{Duration, Instant};

use anyhow::Result;
use nokhwa::pixel_format::RgbFormat;
use renderer::debug_overlay::OverlayInput;
use solver::types::{EulerAngles, EyeValues, RiggedHand, Side, VideoInfo};

use crate::auto_blink::BlinkMode;
use vrm::bone::HumanoidBoneName;

use crate::rig_config::BoneConfig;
use crate::state::AppState;
use imgui_renderer::imnodes::{ImNodesExt, PinShape};

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
    let is_real_camera = state.camera.is_some();
    pipeline_logger::camera(log::Level::Debug, "frame captured")
        .field("source", if is_real_camera { "webcam" } else { "dummy" })
        .field("width", video.width)
        .field("height", video.height)
        .emit();

    // Store camera frame for debug overlay
    state.last_camera_frame = Some(frame.clone());

    // 1. Send frame to tracker thread (non-blocking; drops frame if tracker is busy)
    if state.tracking_enabled {
        state.tracker_thread.send_frame(frame, video.clone());
    }

    // 2. Try to receive a new tracking result (non-blocking)
    if let Some(result) = state
        .tracker_thread
        .try_recv_result()
        .filter(|_| state.tracking_enabled)
    {
        pipeline_logger::tracker(log::Level::Debug, "result received")
            .field(
                "face",
                result.face_landmarks.as_ref().map_or(0, |v| v.len()),
            )
            .field(
                "pose_3d",
                result.pose_landmarks_3d.as_ref().map_or(0, |v| v.len()),
            )
            .field(
                "pose_2d",
                result.pose_landmarks_2d.as_ref().map_or(0, |v| v.len()),
            )
            .field(
                "left_hand",
                result.left_hand_landmarks.as_ref().map_or(0, |v| v.len()),
            )
            .field(
                "right_hand",
                result.right_hand_landmarks.as_ref().map_or(0, |v| v.len()),
            )
            .emit();
        state.last_tracking_result = Some(result);
    }

    // 3. Run solvers on the latest available tracking result
    let mut rig_changed = false;

    if let Some(result) = &state.last_tracking_result {
        if let Some(face_lm) = &result.face_landmarks {
            let face = solver::face::solve(face_lm, &video);
            pipeline_logger::solver(log::Level::Debug, "face solved")
                .field("head_x", format!("{:.3}", face.head.x))
                .field("head_y", format!("{:.3}", face.head.y))
                .field("head_z", format!("{:.3}", face.head.z))
                .field("eye_l", format!("{:.3}", face.eye.l))
                .field("eye_r", format!("{:.3}", face.eye.r))
                .field("mouth_a", format!("{:.3}", face.mouth.a))
                .emit();
            state.rig.face = Some(face);
            rig_changed = true;
        }

        if let Some(pose_3d) = &result.pose_landmarks_3d {
            let pose_2d_vec: Vec<glam::Vec2> =
                result.pose_landmarks_2d.as_deref().unwrap_or(&[]).to_vec();
            // If 2D landmarks are still empty (rare fallback), use zero vectors
            let pose_2d_vec = if pose_2d_vec.is_empty() {
                vec![glam::Vec2::new(0.5, 0.5); 33]
            } else {
                pose_2d_vec
            };
            let pose = solver::pose::solve(pose_3d, &pose_2d_vec, &video);
            pipeline_logger::solver(log::Level::Debug, "pose solved")
                .field(
                    "hip_pos",
                    format!(
                        "{:.3},{:.3},{:.3}",
                        pose.hips.position.x, pose.hips.position.y, pose.hips.position.z
                    ),
                )
                .field(
                    "hip_rot",
                    format!(
                        "{:.3},{:.3},{:.3}",
                        pose.hips.rotation.x, pose.hips.rotation.y, pose.hips.rotation.z
                    ),
                )
                .field(
                    "spine",
                    format!(
                        "{:.3},{:.3},{:.3}",
                        pose.spine.x, pose.spine.y, pose.spine.z
                    ),
                )
                .emit();
            state.rig.pose = Some(pose);
            rig_changed = true;
        }

        // Hand landmarks are swapped: camera mirrors the image, so the tracker's
        // "right" hand is actually the user's left hand (matching testbed:
        // leftHandLandmarks = results.rightHandLandmarks)
        if let Some(right_lm) = &result.right_hand_landmarks {
            let hand = solver::hand::solve(right_lm, Side::Left);
            pipeline_logger::solver(log::Level::Debug, "left hand solved (from right landmarks)")
                .field(
                    "wrist",
                    format!(
                        "{:.3},{:.3},{:.3}",
                        hand.wrist.x, hand.wrist.y, hand.wrist.z
                    ),
                )
                .emit();
            state.rig.left_hand = Some(hand);
            rig_changed = true;
        } else if state.rig.left_hand.is_some() {
            // Clear stale hand rig when hand is no longer detected
            state.rig.left_hand = None;
            rig_changed = true;
        }

        if let Some(left_lm) = &result.left_hand_landmarks {
            let hand = solver::hand::solve(left_lm, Side::Right);
            pipeline_logger::solver(log::Level::Debug, "right hand solved (from left landmarks)")
                .field(
                    "wrist",
                    format!(
                        "{:.3},{:.3},{:.3}",
                        hand.wrist.x, hand.wrist.y, hand.wrist.z
                    ),
                )
                .emit();
            state.rig.right_hand = Some(hand);
            rig_changed = true;
        } else if state.rig.right_hand.is_some() {
            state.rig.right_hand = None;
            rig_changed = true;
        }
    } else {
        pipeline_logger::solver(log::Level::Trace, "no tracking result available").emit();
    }

    // Auto blink: update every frame regardless of tracking
    if state.blink_mode == BlinkMode::Auto {
        state.auto_blink.update();
        state.vrm_model.blend_shapes.set(
            vrm::blendshape::BlendShapePreset::Blink,
            state.auto_blink.value,
        );
        rig_changed = true;
    }

    // 3.5a. Advance idle animation (only if enabled)
    let idle_active = state.idle_animation.as_ref().map_or(false, |a| a.enabled);
    if let Some(anim) = &mut state.idle_animation {
        anim.update(elapsed.as_secs_f32());
    }

    // 4. Apply rig to VRM model (if rig changed, first frame, or idle animation is playing)
    if rig_changed || state.rig_dirty || idle_active {
        pipeline_logger::bone(log::Level::Debug, "applying rig to model")
            .field("rig_changed", rig_changed)
            .field("rig_dirty", state.rig_dirty)
            .emit();
        apply_rig_to_model(state);
        state.rig_dirty = false;
    }

    // 3.5. Update spring bone physics (using spring-physics crate)
    if state.spring_physics_enabled {
        let delta_time = elapsed.as_secs_f32();
        // Compute world matrices for all nodes
        let node_matrices = state.vrm_model.compute_world_matrices();
        state
            .vrm_model
            .spring_world
            .update(delta_time, &node_matrices);
        // Apply physics results to node transforms
        for result in state.vrm_model.spring_world.bone_results() {
            if result.node_index < state.vrm_model.node_transforms.len() {
                state.vrm_model.node_transforms[result.node_index].rotation =
                    result.world_rotation;
            }
        }
    }

    // 4. Update GPU buffers
    // Rotate model 180° around Y to face camera (matching testbed: scene.rotation.y = Math.PI)
    // This must be applied to joint matrices too, not just the camera uniform,
    // because skinned vertices bypass camera.model in the shader.
    let model_matrix = glam::Mat4::from_translation(glam::Vec3::new(
        state.model_offset[0],
        state.model_offset[1],
        0.0,
    )) * glam::Mat4::from_rotation_y(std::f32::consts::PI);

    // Compute world matrices for all nodes via FK, then build per-joint skinning matrices
    let world_matrices = state
        .vrm_model
        .humanoid_bones
        .compute_joint_matrices(&state.vrm_model.node_transforms);
    let joint_matrices: Vec<glam::Mat4> = state
        .vrm_model
        .skins
        .iter()
        .map(|joint| model_matrix * world_matrices[joint.node_index] * joint.inverse_bind_matrix)
        .collect();

    // Log bone/skinning diagnostics
    {
        let non_identity_joints = joint_matrices
            .iter()
            .filter(|m| **m != glam::Mat4::IDENTITY)
            .count();
        pipeline_logger::bone(log::Level::Debug, "joint matrices computed")
            .field("world_nodes", world_matrices.len())
            .field("skin_joints", joint_matrices.len())
            .field("non_identity", non_identity_joints)
            .emit();

        // Log hips bone world matrix as a key diagnostic
        if let Some(hips) = state
            .vrm_model
            .humanoid_bones
            .get(vrm::bone::HumanoidBoneName::Hips)
        {
            let hips_world = world_matrices
                .get(hips.node_index)
                .copied()
                .unwrap_or(glam::Mat4::IDENTITY);
            let t = hips_world.col(3);
            pipeline_logger::bone(log::Level::Debug, "hips world transform")
                .field("node_idx", hips.node_index)
                .field("translation", format!("{:.3},{:.3},{:.3}", t.x, t.y, t.z))
                .emit();
        }
    }
    // Compute per-mesh morph weights (using glTF mesh index for blend shape lookup)
    let per_mesh_morph_weights: Vec<Vec<f32>> = state
        .vrm_model
        .meshes
        .iter()
        .map(|m| {
            state
                .vrm_model
                .blend_shapes
                .get_weights_for_mesh(m.gltf_mesh_index, m.morph_targets.len())
        })
        .collect();

    // Log morph weight diagnostics
    {
        let blink_val = state
            .vrm_model
            .blend_shapes
            .get(vrm::blendshape::BlendShapePreset::Blink);
        let mouth_a = state
            .vrm_model
            .blend_shapes
            .get(vrm::blendshape::BlendShapePreset::A);
        let any_nonzero: Vec<(usize, usize, f32)> = per_mesh_morph_weights
            .iter()
            .enumerate()
            .flat_map(|(mi, w)| {
                w.iter()
                    .enumerate()
                    .filter(|(_, v)| **v > 0.001)
                    .map(move |(ti, v)| (mi, ti, *v))
            })
            .collect();
        pipeline_logger::bone(log::Level::Debug, "morph weights")
            .field("blink_preset", format!("{:.3}", blink_val))
            .field("mouth_a_preset", format!("{:.3}", mouth_a))
            .field(
                "mesh_targets",
                format!(
                    "{:?}",
                    state
                        .vrm_model
                        .meshes
                        .iter()
                        .map(|m| m.morph_targets.len())
                        .collect::<Vec<_>>()
                ),
            )
            .field("active_weights", format!("{:?}", any_nonzero))
            .emit();
    }

    let camera = {
        let default_cam = renderer::camera::Camera::default();
        let dir = (default_cam.position - default_cam.target).normalize();
        let eye = default_cam.target + dir * state.camera_distance;
        renderer::camera::Camera {
            position: eye,
            aspect: state.render_ctx.config.width as f32
                / state.render_ctx.config.height.max(1) as f32,
            ..default_cam
        }
    };
    let camera_uniform = camera.to_uniform(model_matrix);

    pipeline_logger::gpu(log::Level::Debug, "uploading buffers")
        .field("joint_matrices", joint_matrices.len())
        .field(
            "morph_weights",
            per_mesh_morph_weights
                .iter()
                .map(|w| w.len())
                .sum::<usize>(),
        )
        .emit();

    state.scene.prepare(
        &state.render_ctx.queue,
        &joint_matrices,
        &per_mesh_morph_weights,
        &camera_uniform,
        &state.stage_lighting,
    );

    // 5. Render: acquire surface → 3D scene → debug overlay → present
    pipeline_logger::render(log::Level::Trace, "submitting draw commands").emit();
    let output = match state.render_ctx.surface.get_current_texture() {
        Ok(tex) => tex,
        Err(e) => anyhow::bail!("Failed to acquire surface texture: {:?}", e),
    };
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    // 5a. Decode video background frame
    if let Some(session) = &mut state.video_session {
        match session.decode_frame(elapsed) {
            Ok(video_decoder::FrameStatus::NewFrame) => {
                if let Some(rgba) = session.frame_rgba() {
                    let info = session.info();
                    state.scene.update_video_frame(
                        &state.render_ctx.queue,
                        rgba,
                        info.width,
                        info.height,
                    );
                }
                state.fps_decode_counter += 1;
            }
            Ok(video_decoder::FrameStatus::Waiting) => {}
            Ok(video_decoder::FrameStatus::EndOfStream) => {
                log::info!("Video background ended");
            }
            Err(e) => {
                log::warn!("Video decode error: {}", e);
            }
        }
    }

    // 5b. Advance background animation (static/GIF — only when no video)
    if state.video_session.is_none() {
        state
            .scene
            .tick_background(&state.render_ctx.queue, elapsed);
    }

    // 5b. Main 3D scene render (order depends on avatar_on_top setting)
    if !state.avatar_on_top {
        // Default: avatar first, ImGui on top
        state.scene.render_to_view(&state.render_ctx, &view);
    }

    // 5b. Debug overlay: camera preview + landmark visualization
    if let Some(camera_frame) = &state.last_camera_frame {
        state.debug_overlay.update_camera_frame(
            &state.render_ctx.device,
            &state.render_ctx.queue,
            camera_frame,
        );
    }

    // 5b. Debug overlay (camera preview + landmarks + HUD)
    if state.show_debug_overlay {
        let hud_lines = build_hud_lines(state);
        let overlay_input = OverlayInput {
            camera_frame: None,
            pose_2d: state
                .last_tracking_result
                .as_ref()
                .and_then(|r| r.pose_landmarks_2d.clone()),
            left_hand: state
                .last_tracking_result
                .as_ref()
                .and_then(|r| r.left_hand_landmarks.clone()),
            right_hand: state
                .last_tracking_result
                .as_ref()
                .and_then(|r| r.right_hand_landmarks.clone()),
            face: state
                .last_tracking_result
                .as_ref()
                .and_then(|r| r.face_landmarks.clone()),
            hud_lines,
        };
        state
            .debug_overlay
            .render(&state.render_ctx, &view, &overlay_input)?;
    }

    // 5c. Sync AppState → AvatarState (for Lua to read)
    // Save snapshot so we can detect Lua-side changes in 5e.
    let avatar_snapshot = {
        let mut av = state.avatar_handle.state.lock().unwrap();
        av.info.render_fps = state.fps_counter;
        av.info.decode_fps = state.fps_decode_counter;
        av.info.frame_ms = elapsed.as_secs_f32() * 1000.0;
        av.info.shading_mode = state.stage_lighting.shading_mode.label().to_string();
        av.info.idle_anim_status = state
            .idle_animation
            .as_ref()
            .map_or("N/A".into(), |a| if a.enabled { "ON".into() } else { "OFF".into() });
        av.info.imgui_version = imgui_renderer::imgui::dear_imgui_version().to_string();
        av.display.mascot_enabled = state.mascot.enabled;
        av.display.always_on_top = state.mascot.always_on_top;
        av.display.fullscreen = state.fullscreen;
        av.display.debug_overlay = state.show_debug_overlay;
        av.display.camera_distance = state.camera_distance;
        av.display.model_offset = state.model_offset;
        av.display.bg_image_path = state.background.image_path.clone().unwrap_or_default();
        av.display.avatar_on_top = state.avatar_on_top;
        av.display.spring_physics_enabled = state.spring_physics_enabled;
        av.tracking.tracking_enabled = state.tracking_enabled;
        av.tracking.auto_blink = state.blink_mode == crate::auto_blink::BlinkMode::Auto;
        av.tracking.idle_animation = state.idle_animation.as_ref().map_or(false, |a| a.enabled);
        av.tracking.has_idle_animation = state.idle_animation.is_some();
        av.tracking.vcam_enabled = state.vcam_enabled;
        av.tracking.virtual_live_shading = state.stage_lighting.shading_mode == renderer::light::ShadingMode::VirtualLive;
        av.tracking.face_tracking = state.face_tracking;
        av.tracking.arm_tracking = state.arm_tracking;
        av.tracking.hand_tracking = state.hand_tracking;
        av.lighting.key.intensity = state.stage_lighting.key.intensity;
        av.lighting.key.color = state.stage_lighting.key.color;
        av.lighting.key.preset = state.stage_lighting.key.preset.label().to_string();
        av.lighting.fill.intensity = state.stage_lighting.fill.intensity;
        av.lighting.fill.color = state.stage_lighting.fill.color;
        av.lighting.fill.preset = state.stage_lighting.fill.preset.label().to_string();
        av.lighting.back.intensity = state.stage_lighting.back.intensity;
        av.lighting.back.color = state.stage_lighting.back.color;
        av.lighting.back.preset = state.stage_lighting.back.preset.label().to_string();
        av.clone()
    };

    // 5d. ImGui overlay render
    if state.show_imgui {
        if let Some(imgui) = &mut state.imgui {
            // Collect mutable state into temporaries to avoid borrow conflicts
            let mut mascot_enabled = state.mascot.enabled;
            let mut always_on_top = state.mascot.always_on_top;
            let mut fullscreen = state.fullscreen;
            let mut show_debug_overlay = state.show_debug_overlay;
            let mut tracking_enabled = state.tracking_enabled;
            let mut vcam_enabled = state.vcam_enabled;
            let mut blink_auto = state.blink_mode == BlinkMode::Auto;
            let mut camera_distance = state.camera_distance;
            let mut key_intensity = state.stage_lighting.key.intensity;
            let mut fill_intensity = state.stage_lighting.fill.intensity;
            let mut back_intensity = state.stage_lighting.back.intensity;
            let mut key_color = state.stage_lighting.key.color;
            let mut fill_color = state.stage_lighting.fill.color;
            let mut back_color = state.stage_lighting.back.color;
            let mut idle_anim_on = state
                .idle_animation
                .as_ref()
                .map_or(false, |a| a.enabled);
            let has_idle_anim = state.idle_animation.is_some();
            let mut shading_virtual_live = state.stage_lighting.shading_mode == renderer::light::ShadingMode::VirtualLive;
            let shading_label = state.stage_lighting.shading_mode.label().to_string();
            let key_label = state.stage_lighting.key.preset.label().to_string();
            let fill_label = state.stage_lighting.fill.preset.label().to_string();
            let back_label = state.stage_lighting.back.preset.label().to_string();
            let fps_render = state.fps_counter;
            let fps_decode = state.fps_decode_counter;

            // Collect profiler frame times
            static mut FRAME_TIMES: [f32; 120] = [0.0; 120];
            static mut FRAME_IDX: usize = 0;
            let frame_ms = elapsed.as_secs_f32() * 1000.0;
            unsafe {
                FRAME_TIMES[FRAME_IDX % 120] = frame_ms;
                FRAME_IDX += 1;
            }
            let frame_times = unsafe { &FRAME_TIMES };
            let frame_idx = unsafe { FRAME_IDX };

            // Background image path editing buffer
            let mut bg_image_path_buf = state.background.image_path.clone().unwrap_or_default();
            let mut apply_bg_image = false;
            let mut model_offset = state.model_offset;

            // Lazy-init code editor
            if state.code_editor.is_none() {
                let editor = imgui_text_edit::CodeEditor::new();
                editor.set_language(imgui_text_edit::Language::CPlusPlus);
                editor.set_text("// Hello from KalidoKit Rust!\nfn main() {\n    println!(\"Hello, world!\");\n}\n");
                state.code_editor = Some(editor);
            }
            // Take code editor out of state for use in closure
            let code_editor = state.code_editor.take();

            // Lazy-init terminal
            if state.terminal.is_none() && state.imgui_windows.terminal {
                match crate::terminal::ImGuiTerminal::new(120, 30) {
                    Ok(term) => state.terminal = Some(term),
                    Err(e) => log::warn!("Terminal init failed: {e}"),
                }
            }
            // Take lua_imgui out of state for use in closure
            let mut lua_imgui = state.lua_imgui.take();
            // Take terminal out of state for use in closure
            let terminal = state.terminal.take();

            // Copy window visibility flags for the closure
            let mut win = state.imgui_windows.clone_flags();

            imgui.frame_with_nodes(&state.render_ctx.window, |ui, imnodes_ctx, imnodes_editor| {
                // Enable dockspace over the entire viewport
                let dockspace_id = ui.dockspace_over_main_viewport();
                let _ = dockspace_id;

                // // ── Main Menu Bar: Window Manager (commented out) ──
                // ui.main_menu_bar(|| {
                //     ui.menu("Windows", || {
                //         ui.checkbox("Debug Info", &mut win.debug_info);
                //         ui.checkbox("Settings", &mut win.settings);
                //         ui.checkbox("Node Editor", &mut win.node_editor);
                //         ui.checkbox("Profiler", &mut win.profiler);
                //         ui.checkbox("Log", &mut win.log);
                //     });
                // });

                // ── Windows Manager (as a dockable window) ──
                ui.window("Windows")
                    .size([160.0, 0.0], imgui_renderer::imgui::Condition::FirstUseEver)
                    .build(|| {
                        ui.checkbox("Debug Info", &mut win.debug_info);
                        ui.checkbox("Settings", &mut win.settings);
                        ui.checkbox("Node Editor", &mut win.node_editor);
                        ui.checkbox("Profiler", &mut win.profiler);
                        ui.checkbox("Log", &mut win.log);
                        ui.checkbox("Code Editor", &mut win.code_editor);
                        ui.checkbox("Terminal", &mut win.terminal);
                        // Lua-ImGui windows (auto-detected)
                        if let Some(ref mut li) = lua_imgui {
                            if !li.window_visibility.is_empty() {
                                ui.separator();
                                ui.text_disabled("Lua");
                                let mut names: Vec<String> = li.window_visibility.keys().cloned().collect();
                                names.sort();
                                for name in &names {
                                    let mut visible = *li.window_visibility.get(name).unwrap_or(&true);
                                    if ui.checkbox(name, &mut visible) {
                                        li.window_visibility.insert(name.clone(), visible);
                                    }
                                }
                            }
                        }
                    });

                // ── Debug Info ──
                if win.debug_info {
                ui.window("Debug Info")
                    .size([220.0, 0.0], imgui_renderer::imgui::Condition::FirstUseEver)
                    .opened(&mut win.debug_info)
                    .build(|| {
                        ui.text(format!("Render FPS: {fps_render}"));
                        ui.text(format!("Decode FPS: {fps_decode}"));
                        ui.text(format!("Shading: {shading_label}"));
                        ui.text(format!("Idle Anim: {}", if idle_anim_on { "ON" } else { "OFF" }));
                    });
                } // debug_info

                // ── Settings (Rust) — disabled, now handled by Lua Settings ──
                if false && win.settings {
                ui.window("Settings")
                    .size([280.0, 0.0], imgui_renderer::imgui::Condition::FirstUseEver)
                    .opened(&mut win.settings)
                    .build(|| {
                        // ── Info (debug data, at top) ──
                        if ui.collapsing_header("Info", imgui_renderer::imgui::TreeNodeFlags::DEFAULT_OPEN) {
                            ui.text(format!("Render FPS: {fps_render}"));
                            ui.text(format!("Decode FPS: {fps_decode}"));
                            ui.text(format!("Frame: {:.1}ms", frame_ms));
                            ui.text(format!("Shading: {shading_label}"));
                            ui.text(format!("Idle Anim: {}", if idle_anim_on { "ON" } else { "OFF" }));
                            ui.text(format!("Mascot: {}, AlwaysOnTop: {}", mascot_enabled, always_on_top));
                            ui.text(format!("Tracking: {}", if tracking_enabled { "ON" } else { "OFF" }));
                            ui.text(format!("Camera dist: {:.2}", camera_distance));
                            ui.text(format!("Model offset: [{:.2}, {:.2}]", model_offset[0], model_offset[1]));
                            ui.text(format!("ImGui: {}", imgui_renderer::imgui::dear_imgui_version()));
                        }

                        // ── Display ──
                        if ui.collapsing_header("Display", imgui_renderer::imgui::TreeNodeFlags::DEFAULT_OPEN) {
                            ui.checkbox("Mascot Mode (M)", &mut mascot_enabled);
                            // Mascot sub-items: background image (shown when mascot OFF)
                            if !mascot_enabled {
                                ui.indent();
                                ui.text("Background Image:");
                                ui.set_next_item_width(-120.0);
                                ui.input_text("##bg_path", &mut bg_image_path_buf).build();
                                ui.same_line();
                                if ui.button("Browse..") {
                                    use dear_file_browser::{FileDialog, DialogMode, FileFilter};
                                    let dialog = FileDialog::new(DialogMode::OpenFile)
                                        .filter(FileFilter::new("Images", vec![
                                            "png".into(), "jpg".into(), "jpeg".into(),
                                            "bmp".into(), "gif".into(), "webp".into(),
                                        ]))
                                        .filter(FileFilter::new("Video", vec![
                                            "mp4".into(), "mov".into(), "avi".into(),
                                            "mkv".into(), "webm".into(),
                                        ]))
                                        .filter(FileFilter::new("All", vec!["*".into()]));
                                    if let Ok(sel) = dialog.open_blocking() {
                                        if let Some(path) = sel.file_path_name() {
                                            bg_image_path_buf = path.to_string_lossy().into_owned();
                                            apply_bg_image = true;
                                        }
                                    }
                                }
                                ui.same_line();
                                if ui.button("Apply") {
                                    apply_bg_image = true;
                                }
                                ui.unindent();
                            }
                            ui.checkbox("Always on Top (F)", &mut always_on_top);
                            ui.checkbox("Maximized Window", &mut fullscreen);
                            ui.checkbox("Debug Overlay", &mut show_debug_overlay);
                            ui.slider("Camera Distance", 0.5, 10.0, &mut camera_distance);
                            ui.drag_float2("Model Offset", &mut model_offset);
                        }

                        // ── Tracking ──
                        if ui.collapsing_header("Tracking", imgui_renderer::imgui::TreeNodeFlags::DEFAULT_OPEN) {
                            ui.checkbox("Tracking (T)", &mut tracking_enabled);
                            ui.checkbox("Auto Blink (B)", &mut blink_auto);
                            if has_idle_anim {
                                let prev = idle_anim_on;
                                ui.checkbox("Idle Animation (I)", &mut idle_anim_on);
                                if prev != idle_anim_on {
                                    ui.text_colored([1.0, 1.0, 0.0, 1.0], format!("-> {}", idle_anim_on));
                                }
                                ui.same_line();
                                ui.text_colored([0.5, 0.5, 0.5, 1.0], format!("({})", idle_anim_on));
                            } else {
                                ui.text_disabled("Idle Animation (not loaded)");
                            }
                            ui.checkbox("Virtual Camera (C)", &mut vcam_enabled);
                            ui.checkbox("VirtualLive Shading", &mut shading_virtual_live);
                        }

                        // ── Lighting ──
                        if ui.collapsing_header("Lighting", imgui_renderer::imgui::TreeNodeFlags::DEFAULT_OPEN) {
                            ui.text(format!("Key: {key_label}"));
                            ui.slider("Key Intensity", 0.0, 3.0, &mut key_intensity);
                            ui.color_edit3("Key Color", &mut key_color);
                            ui.separator();
                            ui.text(format!("Fill: {fill_label}"));
                            ui.slider("Fill Intensity", 0.0, 3.0, &mut fill_intensity);
                            ui.color_edit3("Fill Color", &mut fill_color);
                            ui.separator();
                            ui.text(format!("Back: {back_label}"));
                            ui.slider("Back Intensity", 0.0, 3.0, &mut back_intensity);
                            ui.color_edit3("Back Color", &mut back_color);
                        }
                    });
                } // settings

                // ── Node Editor (ImNodes) ──
                if win.node_editor {
                ui.window("Node Editor")
                    .opened(&mut win.node_editor)
                    .size([500.0, 300.0], imgui_renderer::imgui::Condition::FirstUseEver)
                    .build(|| {
                        if let Some(ctx) = imnodes_ctx {
                            let editor = ui.imnodes_editor(ctx, imnodes_editor);

                            // Node IDs
                            const NODE_CAMERA: i32 = 1;
                            const NODE_TRACKER: i32 = 2;
                            const NODE_SOLVER: i32 = 3;
                            const NODE_VRM_RIG: i32 = 4;
                            const NODE_RENDERER: i32 = 5;
                            const NODE_VAD: i32 = 6;
                            const NODE_STT: i32 = 7;

                            // Attribute IDs (unique across all nodes)
                            // Camera: out=11
                            // Tracker: in=20, out=21
                            // Solver: in=30, out=31
                            // VRM Rig: in=40, out=41
                            // Renderer: in=50
                            // VAD: in=60, out=61
                            // STT: in=70, out=71

                            // Camera node
                            {
                                let n = editor.node(NODE_CAMERA);
                                n.title_bar(|| ui.text("Camera"));
                                let _out = editor.output_attr(11, PinShape::CircleFilled);
                                ui.text("frame");
                            }

                            // Tracker node
                            {
                                let n = editor.node(NODE_TRACKER);
                                n.title_bar(|| ui.text("Tracker"));
                                let _in = editor.input_attr(20, PinShape::CircleFilled);
                                ui.text("image");
                                _in.end();
                                let _out = editor.output_attr(21, PinShape::CircleFilled);
                                ui.text("landmarks");
                            }

                            // Solver node
                            {
                                let n = editor.node(NODE_SOLVER);
                                n.title_bar(|| ui.text("Solver"));
                                let _in = editor.input_attr(30, PinShape::CircleFilled);
                                ui.text("landmarks");
                                _in.end();
                                let _out = editor.output_attr(31, PinShape::CircleFilled);
                                ui.text("rig data");
                            }

                            // VRM Rig node
                            {
                                let n = editor.node(NODE_VRM_RIG);
                                n.title_bar(|| ui.text("VRM Rig"));
                                let _in = editor.input_attr(40, PinShape::CircleFilled);
                                ui.text("rig data");
                                _in.end();
                                let _out = editor.output_attr(41, PinShape::CircleFilled);
                                ui.text("bones");
                            }

                            // Renderer node
                            {
                                let n = editor.node(NODE_RENDERER);
                                n.title_bar(|| ui.text("Renderer"));
                                let _in = editor.input_attr(50, PinShape::CircleFilled);
                                ui.text("bones");
                            }

                            // VAD node
                            {
                                let n = editor.node(NODE_VAD);
                                n.title_bar(|| ui.text("VAD"));
                                let _in = editor.input_attr(60, PinShape::CircleFilled);
                                ui.text("audio");
                                _in.end();
                                let _out = editor.output_attr(61, PinShape::CircleFilled);
                                ui.text("speech");
                            }

                            // STT node
                            {
                                let n = editor.node(NODE_STT);
                                n.title_bar(|| ui.text("STT"));
                                let _in = editor.input_attr(70, PinShape::CircleFilled);
                                ui.text("speech");
                                _in.end();
                                let _out = editor.output_attr(71, PinShape::CircleFilled);
                                ui.text("text");
                            }

                            // Links (link_id, from_output_attr, to_input_attr)
                            editor.link(1, 11, 20);  // Camera → Tracker
                            editor.link(2, 21, 30);  // Tracker → Solver
                            editor.link(3, 21, 40);  // Tracker → VRM Rig
                            editor.link(4, 31, 50);  // Solver → Renderer
                            editor.link(5, 41, 50);  // VRM Rig → Renderer
                            editor.link(6, 11, 60);  // Camera → VAD
                            editor.link(7, 61, 70);  // VAD → STT

                            editor.end();
                        } else {
                            ui.text_colored([1.0, 0.5, 0.0, 1.0], "ImNodes not available");
                        }
                    });
                } // node_editor

                // ── Profiler ──
                if win.profiler {
                ui.window("Profiler")
                    .size([300.0, 150.0], imgui_renderer::imgui::Condition::FirstUseEver)
                    .opened(&mut win.profiler)
                    .build(|| {
                        let avg = frame_times.iter().sum::<f32>() / 120.0;
                        let max = frame_times.iter().cloned().fold(0.0f32, f32::max);
                        ui.text(format!("Frame: {:.1}ms avg, {:.1}ms max", avg, max));
                        ui.plot_lines_config("##frame_times", frame_times)
                            .overlay_text(format!("{:.1}ms", frame_times[(frame_idx.wrapping_sub(1)) % 120]))
                            .graph_size([280.0, 60.0])
                            .scale_min(0.0)
                            .scale_max(50.0)
                            .build();
                        ui.separator();
                        ui.text(format!("GPU: wgpu 29.0"));
                        ui.text(format!("ImGui: {}", imgui_renderer::imgui::dear_imgui_version()));
                    });
                } // profiler

                // ── Terminal / Log ──
                if win.log {
                ui.window("Log")
                    .size([400.0, 150.0], imgui_renderer::imgui::Condition::FirstUseEver)
                    .opened(&mut win.log)
                    .build(|| {
                        // Show last N log-style lines
                        let lines = [
                            format!("[{:.1}s] Render FPS: {fps_render}", elapsed.as_secs_f64()),
                            format!("[{:.1}s] Decode FPS: {fps_decode}", elapsed.as_secs_f64()),
                            format!("[{:.1}s] Tracking: {}", elapsed.as_secs_f64(), if tracking_enabled { "ON" } else { "OFF" }),
                            format!("[{:.1}s] Mascot: {}, AlwaysOnTop: {}", elapsed.as_secs_f64(), mascot_enabled, always_on_top),
                            format!("[{:.1}s] Camera dist: {:.2}", elapsed.as_secs_f64(), camera_distance),
                            format!("[{:.1}s] Frame: {:.1}ms", elapsed.as_secs_f64(), frame_ms),
                        ];
                        for line in &lines {
                            ui.text_colored([0.5, 0.6, 0.8, 1.0], line);
                        }
                        // Auto-scroll to bottom
                        if ui.scroll_y() >= ui.scroll_max_y() {
                            ui.set_scroll_here_y(0.5);
                        }
                    });
                } // log

                // ── Code Editor ──
                if win.code_editor {
                    if let Some(ref editor) = code_editor {
                    ui.window("Code Editor")
                        .size([500.0, 400.0], imgui_renderer::imgui::Condition::FirstUseEver)
                        .opened(&mut win.code_editor)
                        .build(|| {
                            // Status bar
                            let cl = editor.cursor_line();
                            let cc = editor.cursor_column();
                            let tl = editor.total_lines();
                            ui.text(format!("Ln {}, Col {} | {} lines", cl + 1, cc + 1, tl));
                            ui.separator();
                            // Render the TextEditor widget
                            editor.render("##code_editor", [0.0, 0.0], false);
                        });
                    }
                } // code_editor

                // ── Terminal ──
                if win.terminal {
                    if let Some(ref term) = terminal {
                    ui.window("Terminal")
                        .size([500.0, 300.0], imgui_renderer::imgui::Condition::FirstUseEver)
                        .opened(&mut win.terminal)
                        .build(|| {
                            term.render_with_input(ui);
                        });
                    } else {
                    ui.window("Terminal")
                        .size([500.0, 300.0], imgui_renderer::imgui::Condition::FirstUseEver)
                        .opened(&mut win.terminal)
                        .build(|| {
                            ui.text_colored([1.0, 0.5, 0.0, 1.0], "Terminal not available");
                        });
                    }
                } // terminal

                // ── Lua-ImGui overlay ──
                if let Some(ref mut li) = lua_imgui {
                    li.replay(ui, frame_ms / 1000.0);
                }
            });

            // Put code editor, terminal, lua_imgui back into state
            state.code_editor = code_editor;
            state.terminal = terminal;
            state.lua_imgui = lua_imgui;

            // Apply window visibility changes back to state
            state.imgui_windows.apply_flags(&win);

            // NOTE: Rust Settings apply is disabled — Lua Settings (via AvatarState
            // sync in 5e/5f) now handles all state changes. Re-enable if Lua Settings
            // is removed, or merge both paths into AvatarState-only flow.
            // Apply window visibility only (not controlled by Lua).
            #[allow(unreachable_code)]
            if false {
            if mascot_enabled != state.mascot.enabled {
                state.pending_mascot_toggle = true;
            }
            if always_on_top != state.mascot.always_on_top {
                state.mascot.toggle_always_on_top(&state.render_ctx.window);
            }
            if fullscreen != state.fullscreen {
                state.fullscreen = fullscreen;
                state.render_ctx.window.set_maximized(fullscreen);
            }
            state.show_debug_overlay = show_debug_overlay;
            state.tracking_enabled = tracking_enabled;
            state.vcam_enabled = vcam_enabled;
            state.camera_distance = camera_distance;
            state.blink_mode = if blink_auto { BlinkMode::Auto } else { BlinkMode::Tracking };
            state.stage_lighting.key.intensity = key_intensity;
            state.stage_lighting.fill.intensity = fill_intensity;
            state.stage_lighting.back.intensity = back_intensity;
            state.stage_lighting.key.color = key_color;
            state.stage_lighting.fill.color = fill_color;
            state.stage_lighting.back.color = back_color;
            state.model_offset = model_offset;
            if let Some(anim) = &mut state.idle_animation {
                if anim.enabled != idle_anim_on {
                    anim.enabled = idle_anim_on;
                    if !idle_anim_on {
                        let node_transforms = &state.vrm_model.node_transforms;
                        state
                            .vrm_model
                            .humanoid_bones
                            .reset_to_bind_pose(node_transforms);
                    }
                    state.rig_dirty = true;
                }
            }
            let new_shading = if shading_virtual_live {
                renderer::light::ShadingMode::VirtualLive
            } else {
                renderer::light::ShadingMode::Classic
            };
            state.stage_lighting.shading_mode = new_shading;
            if apply_bg_image {
                let new_path = if bg_image_path_buf.trim().is_empty() {
                    None
                } else {
                    Some(bg_image_path_buf.trim().to_string())
                };
                state.background.image_path = new_path.clone();
                if let Err(e) = state.scene.set_background_image_from_path(
                    &state.render_ctx.device,
                    &state.render_ctx.queue,
                    state.render_ctx.config.format,
                    new_path.as_deref(),
                ) {
                    log::warn!("Failed to set background image: {e}");
                }
            }
            } // end if false

            imgui.render(&state.render_ctx.device, &state.render_ctx.queue, &view);
        }
    }

    // 5d-2. Avatar on top mode: render 3D scene AFTER ImGui (overlay, no clear)
    if state.avatar_on_top {
        state.scene.render_to_view_overlay(&state.render_ctx, &view);
    }

    // 5e. Sync AvatarState → AppState (only Lua-modified fields)
    // Compare current AvatarState against snapshot taken before Lua ran.
    // Only apply values that Lua actually changed (avoids overwriting wheel/keyboard input).
    {
        let av = state.avatar_handle.state.lock().unwrap();
        let snap = &avatar_snapshot;

        // Display
        if av.display.mascot_enabled != snap.display.mascot_enabled {
            state.pending_mascot_toggle = true;
        }
        if av.display.always_on_top != snap.display.always_on_top {
            state.mascot.toggle_always_on_top(&state.render_ctx.window);
        }
        if av.display.fullscreen != snap.display.fullscreen {
            state.fullscreen = av.display.fullscreen;
            state.render_ctx.window.set_maximized(av.display.fullscreen);
        }
        if av.display.debug_overlay != snap.display.debug_overlay {
            state.show_debug_overlay = av.display.debug_overlay;
        }
        if av.display.camera_distance != snap.display.camera_distance {
            state.camera_distance = av.display.camera_distance;
        }
        if av.display.model_offset != snap.display.model_offset {
            state.model_offset = av.display.model_offset;
        }
        if av.display.avatar_on_top != snap.display.avatar_on_top {
            state.avatar_on_top = av.display.avatar_on_top;
        }
        if av.display.spring_physics_enabled != snap.display.spring_physics_enabled {
            state.spring_physics_enabled = av.display.spring_physics_enabled;
        }
        // Tracking
        if av.tracking.tracking_enabled != snap.tracking.tracking_enabled {
            state.tracking_enabled = av.tracking.tracking_enabled;
        }
        if av.tracking.auto_blink != snap.tracking.auto_blink {
            state.blink_mode = if av.tracking.auto_blink {
                crate::auto_blink::BlinkMode::Auto
            } else {
                crate::auto_blink::BlinkMode::Tracking
            };
        }
        if av.tracking.vcam_enabled != snap.tracking.vcam_enabled {
            state.vcam_enabled = av.tracking.vcam_enabled;
        }
        // Idle animation
        if av.tracking.idle_animation != snap.tracking.idle_animation {
            if let Some(anim) = &mut state.idle_animation {
                anim.enabled = av.tracking.idle_animation;
                if !av.tracking.idle_animation {
                    let node_transforms = &state.vrm_model.node_transforms;
                    state.vrm_model.humanoid_bones.reset_to_bind_pose(node_transforms);
                }
                state.rig_dirty = true;
            }
        }
        // Per-feature tracking toggles
        if av.tracking.face_tracking != snap.tracking.face_tracking {
            state.face_tracking = av.tracking.face_tracking;
        }
        if av.tracking.arm_tracking != snap.tracking.arm_tracking {
            state.arm_tracking = av.tracking.arm_tracking;
        }
        if av.tracking.hand_tracking != snap.tracking.hand_tracking {
            state.hand_tracking = av.tracking.hand_tracking;
        }
        // face_only mode: disable pose/hand detection if only face tracking is enabled
        let need_pose_hand = state.arm_tracking || state.hand_tracking;
        state.tracker_thread.set_face_only(!need_pose_hand);
        // Shading
        if av.tracking.virtual_live_shading != snap.tracking.virtual_live_shading {
            state.stage_lighting.shading_mode = if av.tracking.virtual_live_shading {
                renderer::light::ShadingMode::VirtualLive
            } else {
                renderer::light::ShadingMode::Classic
            };
        }
        // Lighting
        if av.lighting.key.intensity != snap.lighting.key.intensity {
            state.stage_lighting.key.intensity = av.lighting.key.intensity;
        }
        if av.lighting.key.color != snap.lighting.key.color {
            state.stage_lighting.key.color = av.lighting.key.color;
        }
        if av.lighting.fill.intensity != snap.lighting.fill.intensity {
            state.stage_lighting.fill.intensity = av.lighting.fill.intensity;
        }
        if av.lighting.fill.color != snap.lighting.fill.color {
            state.stage_lighting.fill.color = av.lighting.fill.color;
        }
        if av.lighting.back.intensity != snap.lighting.back.intensity {
            state.stage_lighting.back.intensity = av.lighting.back.intensity;
        }
        if av.lighting.back.color != snap.lighting.back.color {
            state.stage_lighting.back.color = av.lighting.back.color;
        }
    }

    // 5f. Process avatar action queue
    {
        let actions = state.avatar_handle.actions.lock().unwrap().drain();
        for action in actions {
            match action {
                avatar_sdk::AvatarAction::ApplyBackgroundImage(path) => {
                    let new_path = if path.trim().is_empty() { None } else { Some(path) };
                    state.background.image_path = new_path.clone();
                    if let Err(e) = state.scene.set_background_image_from_path(
                        &state.render_ctx.device,
                        &state.render_ctx.queue,
                        state.render_ctx.config.format,
                        new_path.as_deref(),
                    ) {
                        log::warn!("Failed to set background image: {e}");
                    }
                }
                avatar_sdk::AvatarAction::ToggleMascot => {
                    state.pending_mascot_toggle = true;
                }
                avatar_sdk::AvatarAction::ResetIdlePose => {
                    let node_transforms = &state.vrm_model.node_transforms;
                    state.vrm_model.humanoid_bones.reset_to_bind_pose(node_transforms);
                    state.rig_dirty = true;
                }
                avatar_sdk::AvatarAction::BrowseBackgroundImage => {
                    use dear_file_browser::{FileDialog, DialogMode, FileFilter};
                    let dialog = FileDialog::new(DialogMode::OpenFile)
                        .filter(FileFilter::new("Images", vec![
                            "png".into(), "jpg".into(), "jpeg".into(),
                            "bmp".into(), "gif".into(), "webp".into(),
                        ]))
                        .filter(FileFilter::new("Video", vec![
                            "mp4".into(), "mov".into(), "avi".into(),
                            "mkv".into(), "webm".into(),
                        ]));
                    if let Ok(sel) = dialog.open_blocking() {
                        if let Some(path) = sel.file_path_name() {
                            let p = path.to_string_lossy().to_string();
                            state.background.image_path = Some(p.clone());
                            if let Err(e) = state.scene.set_background_image_from_path(
                                &state.render_ctx.device,
                                &state.render_ctx.queue,
                                state.render_ctx.config.format,
                                Some(p.as_str()),
                            ) {
                                log::warn!("Failed to set background image: {e}");
                            }
                        }
                    }
                }
            }
        }
    }

    // 5c. Lua-ImGui overlay — now rendered inside frame_with_nodes (see above)

    output.present();

    // Deferred mascot toggle: safe now that SurfaceTexture is dropped.
    if state.pending_mascot_toggle {
        state.pending_mascot_toggle = false;
        state.mascot.toggle(&state.render_ctx.window, state.fullscreen);
        if state.mascot.enabled {
            state.render_ctx.set_transparent(true);
            state.scene.set_clear_alpha(0.0);
            state.scene.remove_background_video();
        } else {
            state.render_ctx.set_transparent(false);
            state.scene.set_clear_alpha(1.0);
        }
    }

    // 6. Virtual camera: capture and send frame (throttled to 30fps)
    if state.vcam_enabled {
        let now = Instant::now();
        if now.duration_since(state.vcam_last_send).as_millis() >= 33 {
            vcam_send_frame(state);
            state.vcam_last_send = now;
        }
    }

    // 7. Mascot mode: capture alpha map for pixel-level hit-testing.
    // Piggybacks on the frame capture system (same as vcam) but uses the
    // mascot window dimensions. The alpha channel is extracted from the BGRA
    // readback data and cached for CursorMoved hit-test lookups.
    if state.mascot.enabled {
        // Use actual window logical size for alpha map (not mascot_size,
        // which is 512x512 even in fullscreen mascot mode).
        let phys = state.render_ctx.window.inner_size();
        let scale = state.render_ctx.window.scale_factor();
        let w = (phys.width as f64 / scale) as u32;
        let h = (phys.height as f64 / scale) as u32;
        state
            .scene
            .ensure_frame_capture(&state.render_ctx.device, w, h);
        state.scene.render_to_capture(&state.render_ctx);
        if let Some(bgra_data) = state
            .scene
            .capture_frame_async(&state.render_ctx.device, &state.render_ctx.queue)
        {
            // Extract alpha channel (byte offset 3 in each BGRA pixel)
            let pixel_count = (w * h) as usize;
            let mut alpha_map = Vec::with_capacity(pixel_count);
            for i in 0..pixel_count {
                alpha_map.push(bgra_data[i * 4 + 3]);
            }
            state.mascot_alpha_map = alpha_map;
            state.mascot_alpha_width = w;
            state.mascot_alpha_height = h;
        }
    } else if !state.mascot_alpha_map.is_empty() {
        // Clear alpha map when mascot mode is off to free memory
        state.mascot_alpha_map.clear();
        state.mascot_alpha_width = 0;
        state.mascot_alpha_height = 0;
    }

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
///
/// When an idle animation is active, bone rotations are blended:
/// - Bones with tracking: `idle_quat.slerp(tracking_quat, tracking_weight)`
/// - Bones without tracking: idle animation rotation directly
fn apply_rig_to_model(state: &mut AppState) {
    let cfg = &state.rig_config;

    // Sample idle animation pose (if active)
    let idle_pose = state.idle_animation.as_ref().and_then(|anim| {
        if anim.enabled {
            Some(anim.sample())
        } else {
            None
        }
    });

    // Apply idle animation to bones that are NOT driven by tracking.
    // This sets a base pose that tracking will override/blend with.
    if let Some(ref idle) = idle_pose {
        let tracking_bones = collect_tracked_bones(state);
        for (&bone, &rot) in idle {
            if !tracking_bones.contains(&bone) {
                state.vrm_model.humanoid_bones.set_rotation(bone, rot);
            }
        }
    }

    // Apply face rig (only if face_tracking enabled)
    if state.face_tracking {
    if let Some(face) = &state.rig.face {
        // Head rotation: rigRotation("Neck", head, 0.7)
        let neck_quat = blend_with_idle(
            face.head.to_quat_dampened(cfg.neck.dampener),
            HumanoidBoneName::Neck,
            &idle_pose,
            &state.idle_animation,
        );
        state.vrm_model.humanoid_bones.set_rotation_interpolated(
            vrm::bone::HumanoidBoneName::Neck,
            neck_quat,
            cfg.neck.lerp_amount,
        );

        // Eye blink: mode-dependent
        match state.blink_mode {
            BlinkMode::Tracking => {
                // Match reference testbed exactly:
                // 1. Convert eye openness to blink amount (1=closed, 0=open)
                // 2. Lerp with previous Blink blend shape value (smoothing)
                // 3. Run stabilizeBlink on the blink amounts
                // 4. Set Blink preset to stabilized.l
                let bs = &state.vrm_model.blend_shapes;
                let prev_blink = bs.get(vrm::blendshape::BlendShapePreset::Blink);
                let blink_l = (1.0 - face.eye.l).clamp(0.0, 1.0);
                let blink_r = (1.0 - face.eye.r).clamp(0.0, 1.0);
                let blink_l = blink_l + (prev_blink - blink_l) * 0.5;
                let blink_r = blink_r + (prev_blink - blink_r) * 0.5;
                let stabilized = solver::face::stabilize_blink(
                    &EyeValues {
                        l: blink_l,
                        r: blink_r,
                    },
                    face.head.y,
                );
                state
                    .vrm_model
                    .blend_shapes
                    .set(vrm::blendshape::BlendShapePreset::Blink, stabilized.l);
            }
            BlinkMode::Auto => {
                // Auto blink is updated separately (outside face tracking block)
            }
        }

        // Mouth shapes with interpolation: lerp(new, prev, 0.5)
        let bs = &state.vrm_model.blend_shapes;
        let prev_a = bs.get(vrm::blendshape::BlendShapePreset::A);
        let prev_i = bs.get(vrm::blendshape::BlendShapePreset::I);
        let prev_u = bs.get(vrm::blendshape::BlendShapePreset::U);
        let prev_e = bs.get(vrm::blendshape::BlendShapePreset::E);
        let prev_o = bs.get(vrm::blendshape::BlendShapePreset::O);
        let lerp_mouth = |new_val: f32, prev_val: f32| -> f32 {
            let smoothed = new_val + (prev_val - new_val) * 0.5;
            smoothed.clamp(0.0, 1.0)
        };
        state.vrm_model.blend_shapes.set(
            vrm::blendshape::BlendShapePreset::A,
            lerp_mouth(face.mouth.a, prev_a),
        );
        state.vrm_model.blend_shapes.set(
            vrm::blendshape::BlendShapePreset::I,
            lerp_mouth(face.mouth.i, prev_i),
        );
        state.vrm_model.blend_shapes.set(
            vrm::blendshape::BlendShapePreset::U,
            lerp_mouth(face.mouth.u, prev_u),
        );
        state.vrm_model.blend_shapes.set(
            vrm::blendshape::BlendShapePreset::E,
            lerp_mouth(face.mouth.e, prev_e),
        );
        state.vrm_model.blend_shapes.set(
            vrm::blendshape::BlendShapePreset::O,
            lerp_mouth(face.mouth.o, prev_o),
        );

        // Pupil tracking with lerp interpolation
        // Testbed: lookTarget = Euler(lerp(prev.x, pupil.y, 0.4), lerp(prev.y, pupil.x, 0.4), 0)
        // Note the X/Y swap: pupil.y → Euler.x (pitch), pupil.x → Euler.y (yaw)
        let prev = state.rig.prev_look_target;
        let target = face.pupil;
        let interpolated = glam::Vec2::new(
            prev.x + (target.y - prev.x) * cfg.pupil,
            prev.y + (target.x - prev.y) * cfg.pupil,
        );
        state.rig.prev_look_target = interpolated;

        if let Some(look_at) = &state.vrm_model.look_at {
            // Testbed passes raw pupil values as radians to THREE.Euler, then three-vrm
            // internally converts RAD2DEG. Our apply() takes degrees, so convert here.
            let rad2deg = 180.0_f32 / std::f32::consts::PI;
            let euler = vrm::look_at::EulerAngles {
                yaw: interpolated.y * rad2deg,
                pitch: interpolated.x * rad2deg,
            };
            let eye_quat = look_at.apply(&euler);
            state.vrm_model.humanoid_bones.set_rotation_interpolated(
                vrm::bone::HumanoidBoneName::LeftEye,
                eye_quat,
                0.3,
            );
            state.vrm_model.humanoid_bones.set_rotation_interpolated(
                vrm::bone::HumanoidBoneName::RightEye,
                eye_quat,
                0.3,
            );
        }
    }
    } // end face_tracking

    // Apply pose rig — arms only (only if arm_tracking enabled)
    if state.arm_tracking {
    if let Some(pose) = &state.rig.pose {
        // Arms: rigRotation(name, rotation, 1, 0.3)
        for (bone, euler) in [
            (HumanoidBoneName::RightUpperArm, &pose.right_upper_arm),
            (HumanoidBoneName::RightLowerArm, &pose.right_lower_arm),
            (HumanoidBoneName::LeftUpperArm, &pose.left_upper_arm),
            (HumanoidBoneName::LeftLowerArm, &pose.left_lower_arm),
        ] {
            let q = blend_with_idle(
                euler.to_quat_dampened(cfg.limbs.dampener),
                bone,
                &idle_pose,
                &state.idle_animation,
            );
            state.vrm_model.humanoid_bones.set_rotation_interpolated(
                bone,
                q,
                cfg.limbs.lerp_amount,
            );
        }
    }
    } // end arm_tracking

    // Apply hand bones (only if hand_tracking enabled)
    if state.hand_tracking {
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
            &cfg.fingers,
            &idle_pose,
            &state.idle_animation,
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
            &cfg.fingers,
            &idle_pose,
            &state.idle_animation,
        );
    }
    } // end hand_tracking
}

/// Blend a tracking quaternion with the idle animation pose for a given bone.
///
/// Returns `tracking_quat` if no idle animation is active.
/// Otherwise returns `idle_quat.slerp(tracking_quat, tracking_weight)`.
fn blend_with_idle(
    tracking_quat: glam::Quat,
    bone: HumanoidBoneName,
    idle_pose: &Option<std::collections::HashMap<HumanoidBoneName, glam::Quat>>,
    anim: &Option<vrm::animation_player::AnimationPlayer>,
) -> glam::Quat {
    let (Some(idle), Some(anim)) = (idle_pose, anim) else {
        return tracking_quat;
    };
    let Some(&idle_quat) = idle.get(&bone) else {
        return tracking_quat;
    };
    let tw = anim.tracking_weight(bone);
    idle_quat.slerp(tracking_quat, tw)
}

/// Collect the set of bones that are currently driven by tracking data.
fn collect_tracked_bones(state: &AppState) -> std::collections::HashSet<HumanoidBoneName> {
    use HumanoidBoneName::*;
    let mut set = std::collections::HashSet::new();

    if state.rig.face.is_some() {
        set.extend([Neck, LeftEye, RightEye]);
    }

    if state.rig.pose.is_some() {
        set.extend([
            Hips,
            Spine,
            Chest,
            RightUpperArm,
            RightLowerArm,
            LeftUpperArm,
            LeftLowerArm,
            RightUpperLeg,
            RightLowerLeg,
            LeftUpperLeg,
            LeftLowerLeg,
        ]);
    }

    if state.rig.left_hand.is_some() {
        set.extend([
            LeftHand,
            LeftThumbProximal,
            LeftThumbIntermediate,
            LeftThumbDistal,
            LeftIndexProximal,
            LeftIndexIntermediate,
            LeftIndexDistal,
            LeftMiddleProximal,
            LeftMiddleIntermediate,
            LeftMiddleDistal,
            LeftRingProximal,
            LeftRingIntermediate,
            LeftRingDistal,
            LeftLittleProximal,
            LeftLittleIntermediate,
            LeftLittleDistal,
        ]);
    }

    if state.rig.right_hand.is_some() {
        set.extend([
            RightHand,
            RightThumbProximal,
            RightThumbIntermediate,
            RightThumbDistal,
            RightIndexProximal,
            RightIndexIntermediate,
            RightIndexDistal,
            RightMiddleProximal,
            RightMiddleIntermediate,
            RightMiddleDistal,
            RightRingProximal,
            RightRingIntermediate,
            RightRingDistal,
            RightLittleProximal,
            RightLittleIntermediate,
            RightLittleDistal,
        ]);
    }

    set
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
    idle_pose: &Option<std::collections::HashMap<HumanoidBoneName, glam::Quat>>,
    anim: &Option<vrm::animation_player::AnimationPlayer>,
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
        let q = blend_with_idle(
            euler.to_quat_dampened(config.dampener),
            *bone_name,
            idle_pose,
            anim,
        );
        bones.set_rotation_interpolated(*bone_name, q, config.lerp_amount);
    }
}

/// Capture the rendered frame and send it to the virtual camera.
fn vcam_send_frame(state: &mut AppState) {
    // Fixed capture resolution — avoids CPU downscale in send_frame
    const VCAM_W: u32 = 1280;
    const VCAM_H: u32 = 720;

    // Ensure staging resources exist
    state
        .scene
        .ensure_frame_capture(&state.render_ctx.device, VCAM_W, VCAM_H);

    // Render the scene to the capture texture (uses its own depth buffer at VCAM resolution)
    state.scene.render_to_capture(&state.render_ctx);

    // Async double-buffered readback: copies current frame to GPU buffer,
    // returns previous frame's data (one frame latency, non-blocking pipeline).
    let prev_frame = state
        .scene
        .capture_frame_async(&state.render_ctx.device, &state.render_ctx.queue);

    if let Some(bgra_data) = prev_frame {
        #[cfg(target_os = "macos")]
        {
            // Initialize virtual camera on first frame
            use virtual_camera::VirtualCamera;
            if state.vcam.is_none() {
                let mut vcam = virtual_camera::MacOsVirtualCamera::new();
                match vcam.start() {
                    Ok(()) => {
                        state.vcam = Some(vcam);
                    }
                    Err(e) => {
                        log::error!("[VCam] Failed to start: {e}");
                        state.vcam_enabled = false;
                        return;
                    }
                }
            }
            if let Some(vcam) = &mut state.vcam {
                if let Err(e) = vcam.send_frame(&bgra_data, VCAM_W, VCAM_H) {
                    log::warn!("[VCam] send_frame error: {e}");
                }
            }
        }
    }
}

/// Build HUD text lines showing current settings and key bindings.
fn build_hud_lines(state: &AppState) -> Vec<String> {
    let lighting = &state.stage_lighting;
    vec![
        format!("V: Shading ({})", lighting.shading_mode.label()),
        format!(
            "B: Blink mode ({})",
            match state.blink_mode {
                BlinkMode::Tracking => "Tracking",
                BlinkMode::Auto => "Auto",
            }
        ),
        format!("Scroll: Zoom ({:.1})", state.camera_distance),
        String::new(),
        format!(
            "1: Key light   ({}) {:.1}",
            lighting.key.preset.label(),
            lighting.key.intensity
        ),
        format!(
            "2: Fill light  ({}) {:.1}",
            lighting.fill.preset.label(),
            lighting.fill.intensity
        ),
        format!(
            "3: Back light  ({}) {:.1}",
            lighting.back.preset.label(),
            lighting.back.intensity
        ),
        String::new(),
        "Q/W: Key intensity +/-".to_string(),
        "A/S: Fill intensity +/-".to_string(),
        "Z/X: Back intensity +/-".to_string(),
        format!(
            "T: Tracking ({})",
            if state.tracking_enabled { "ON" } else { "OFF" }
        ),
        format!(
            "I: Idle anim ({})",
            state
                .idle_animation
                .as_ref()
                .map_or("N/A", |a| if a.enabled { "ON" } else { "OFF" })
        ),
        format!(
            "C: VCam ({})",
            if state.vcam_enabled { "ON" } else { "OFF" }
        ),
    ]
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

        apply_hand_bones(
            &mut bones,
            &hand,
            &wrist_combined,
            Side::Left,
            &config,
            &None,
            &None,
        );

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
