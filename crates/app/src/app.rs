use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use std::sync::Arc;

use crate::auto_blink::BlinkMode;
use crate::state::AppState;
use crate::user_prefs::UserPrefs;

pub struct App {
    state: Option<AppState>,
}

impl App {
    pub fn new() -> Self {
        Self { state: None }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("KalidoKit Rust - VRM Motion Capture")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
            .with_transparent(true);
        let window = Arc::new(event_loop.create_window(attrs).unwrap());

        match pollster::block_on(crate::init::init_all(window)) {
            Ok(app_state) => {
                // Request initial redraw to kick-start the render loop
                app_state.render_ctx.window.request_redraw();
                self.state = Some(app_state);
            }
            Err(e) => {
                log::error!("Failed to initialize application: {e}");
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.render_ctx.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                save_prefs(state);
                log::info!("User prefs saved on exit");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                state.render_ctx.resize(size.width, size.height);
                state
                    .scene
                    .resize(&state.render_ctx.device, size.width, size.height);
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = crate::update::update_frame(state) {
                    log::error!("Frame update error: {e}");
                }

                // FPS counter: update window title every second
                state.fps_counter += 1;
                let elapsed = state.fps_timer.elapsed();
                if elapsed >= std::time::Duration::from_secs(1) {
                    let render_fps = state.fps_counter as f64 / elapsed.as_secs_f64();
                    let decode_fps = state.fps_decode_counter;
                    state.fps_counter = 0;
                    state.fps_decode_counter = 0;
                    state.fps_timer = std::time::Instant::now();

                    let video_info = state
                        .video_session
                        .as_ref()
                        .map(|s| {
                            format!(
                                " | video decode: {} fps ({:?})",
                                decode_fps,
                                s.info().backend
                            )
                        })
                        .unwrap_or_default();

                    let title = format!(
                        "KalidoKit Rust | render: {:.0} fps{}",
                        render_fps, video_info,
                    );
                    log::info!("{}", title);
                    state.render_ctx.window.set_title(&title);
                }

                state.render_ctx.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.01,
                };
                // Scroll up (positive y) zooms in (decrease distance),
                // scroll down (negative y) zooms out (increase distance).
                state.camera_distance = (state.camera_distance - scroll_y * 0.3).clamp(0.5, 10.0);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(key) = event.physical_key {
                        match key {
                            KeyCode::KeyB => {
                                state.blink_mode = match state.blink_mode {
                                    BlinkMode::Tracking => BlinkMode::Auto,
                                    BlinkMode::Auto => BlinkMode::Tracking,
                                };
                                log::info!("Blink mode: {:?}", state.blink_mode);
                            }
                            // Shading mode toggle
                            KeyCode::KeyV => {
                                state.stage_lighting.shading_mode =
                                    state.stage_lighting.shading_mode.toggle();
                                log::info!(
                                    "Shading mode: {}",
                                    state.stage_lighting.shading_mode.label()
                                );
                            }
                            // Light position cycling: 1=key, 2=fill, 3=back
                            KeyCode::Digit1 => {
                                state.stage_lighting.key.next_preset();
                                log::info!(
                                    "Key light position: {}",
                                    state.stage_lighting.key.preset.label()
                                );
                            }
                            KeyCode::Digit2 => {
                                state.stage_lighting.fill.next_preset();
                                log::info!(
                                    "Fill light position: {}",
                                    state.stage_lighting.fill.preset.label()
                                );
                            }
                            KeyCode::Digit3 => {
                                state.stage_lighting.back.next_preset();
                                log::info!(
                                    "Back light position: {}",
                                    state.stage_lighting.back.preset.label()
                                );
                            }
                            // Intensity adjustment: Q/W=key, A/S=fill, Z/X=back
                            KeyCode::KeyQ => {
                                state.stage_lighting.key.intensity =
                                    (state.stage_lighting.key.intensity + 0.2).min(3.0);
                                log::info!(
                                    "Key light intensity: {:.1}",
                                    state.stage_lighting.key.intensity
                                );
                            }
                            KeyCode::KeyW => {
                                state.stage_lighting.key.intensity =
                                    (state.stage_lighting.key.intensity - 0.2).max(0.0);
                                log::info!(
                                    "Key light intensity: {:.1}",
                                    state.stage_lighting.key.intensity
                                );
                            }
                            KeyCode::KeyA => {
                                state.stage_lighting.fill.intensity =
                                    (state.stage_lighting.fill.intensity + 0.2).min(3.0);
                                log::info!(
                                    "Fill light intensity: {:.1}",
                                    state.stage_lighting.fill.intensity
                                );
                            }
                            KeyCode::KeyS => {
                                state.stage_lighting.fill.intensity =
                                    (state.stage_lighting.fill.intensity - 0.2).max(0.0);
                                log::info!(
                                    "Fill light intensity: {:.1}",
                                    state.stage_lighting.fill.intensity
                                );
                            }
                            KeyCode::KeyZ => {
                                state.stage_lighting.back.intensity =
                                    (state.stage_lighting.back.intensity + 0.2).min(3.0);
                                log::info!(
                                    "Back light intensity: {:.1}",
                                    state.stage_lighting.back.intensity
                                );
                            }
                            KeyCode::KeyX => {
                                state.stage_lighting.back.intensity =
                                    (state.stage_lighting.back.intensity - 0.2).max(0.0);
                                log::info!(
                                    "Back light intensity: {:.1}",
                                    state.stage_lighting.back.intensity
                                );
                            }
                            KeyCode::KeyC => {
                                state.vcam_enabled = !state.vcam_enabled;
                                log::info!(
                                    "Virtual camera: {}",
                                    if state.vcam_enabled { "ON" } else { "OFF" }
                                );
                            }
                            KeyCode::KeyT => {
                                state.tracking_enabled = !state.tracking_enabled;
                                if !state.tracking_enabled {
                                    state.rig = crate::state::RigState::default();
                                    let node_transforms = &state.vrm_model.node_transforms;
                                    state
                                        .vrm_model
                                        .humanoid_bones
                                        .reset_to_bind_pose(node_transforms);
                                    state.rig_dirty = true;
                                }
                                log::info!(
                                    "Tracking: {}",
                                    if state.tracking_enabled { "ON" } else { "OFF" }
                                );
                            }
                            KeyCode::KeyP => {
                                if let Some(session) = &mut state.video_session {
                                    if session.is_paused() {
                                        session.resume();
                                        log::info!("Video background: resumed");
                                    } else {
                                        session.pause();
                                        log::info!("Video background: paused");
                                    }
                                }
                            }
                            KeyCode::KeyI => {
                                if let Some(anim) = &mut state.idle_animation {
                                    anim.enabled = !anim.enabled;
                                    if !anim.enabled {
                                        // Reset bones to bind pose when disabling idle animation
                                        let node_transforms = &state.vrm_model.node_transforms;
                                        state
                                            .vrm_model
                                            .humanoid_bones
                                            .reset_to_bind_pose(node_transforms);
                                    }
                                    log::info!(
                                        "Idle animation: {}",
                                        if anim.enabled { "ON" } else { "OFF" }
                                    );
                                    state.rig_dirty = true;
                                }
                            }
                            KeyCode::KeyM => {
                                state.mascot.toggle(&state.render_ctx.window);
                                if state.mascot.enabled {
                                    state.render_ctx.set_transparent(true);
                                    state.scene.set_clear_alpha(0.0);
                                    state.scene.remove_background_video();
                                } else {
                                    state.render_ctx.set_transparent(false);
                                    state.scene.set_clear_alpha(1.0);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.last_cursor_pos = position;
                if state.mascot.is_dragging() {
                    state.mascot.update_drag(&state.render_ctx.window, position);
                }
            }
            WindowEvent::MouseInput {
                state: btn_state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if state.mascot.enabled {
                    match btn_state {
                        ElementState::Pressed => {
                            state
                                .mascot
                                .start_drag(&state.render_ctx.window, state.last_cursor_pos);
                        }
                        ElementState::Released => {
                            state.mascot.end_drag();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn save_prefs(state: &AppState) {
    UserPrefs {
        camera_distance: state.camera_distance,
        blink_mode: state.blink_mode,
        stage_lighting: state.stage_lighting.clone(),
        animation_path: state.animation_path.clone(),
        background: state.background.clone(),
        mascot_mode: state.mascot.enabled,
    }
    .save();
}
