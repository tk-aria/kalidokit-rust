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
                // Apply fullscreen if configured
                if app_state.fullscreen {
                    app_state.render_ctx.window.set_fullscreen(Some(
                        winit::window::Fullscreen::Borderless(None),
                    ));
                }
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
        if let Some(state) = &mut self.state {
            // Notify ImGui of non-window events
            if state.show_imgui {
                if let Some(imgui) = &mut state.imgui {
                    imgui.handle_non_window_event(
                        &state.render_ctx.window,
                        &winit::event::Event::<()>::AboutToWait,
                    );
                }
            }
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

        // Forward events to ImGui first so it can capture mouse/keyboard
        if let Some(imgui) = &mut state.imgui {
            if state.show_imgui {
                imgui.handle_event(&state.render_ctx.window, _window_id, &event);
            }
        }

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
                // Update ImGui display size to match new surface dimensions
                if let Some(imgui) = &mut state.imgui {
                    imgui.resize(
                        size.width,
                        size.height,
                        state.render_ctx.window.scale_factor(),
                    );
                }
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
                            KeyCode::F1 => {
                                state.show_imgui = !state.show_imgui;
                                log::info!(
                                    "ImGui UI: {}",
                                    if state.show_imgui { "ON" } else { "OFF" }
                                );
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
                            KeyCode::KeyF => {
                                if state.mascot.enabled {
                                    state.mascot.toggle_always_on_top(&state.render_ctx.window);
                                    log::info!("Always on top: {}", state.mascot.always_on_top);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.last_cursor_pos = position;
                // Pixel-alpha hit-testing for mascot mode: if the cursor is over a
                // rendered pixel (alpha > 0) the window captures mouse events (drag,
                // scroll, click). Over transparent areas (alpha == 0) clicks pass
                // through to background windows. No modifier key needed.
                if state.mascot.enabled && !state.mascot_alpha_map.is_empty() {
                    // Convert physical cursor position to alpha map coordinates.
                    // The alpha map uses the mascot window's logical size; the cursor
                    // position is in physical pixels, so scale by the window's scale factor.
                    let scale = state.render_ctx.window.scale_factor();
                    let lx = (position.x / scale) as u32;
                    let ly = (position.y / scale) as u32;
                    // Check if ImGui wants the mouse (cursor is over an ImGui window)
                    let imgui_wants_mouse = state.show_imgui
                        && state.imgui.as_ref().map_or(false, |im| im.want_capture_mouse());

                    if imgui_wants_mouse {
                        // ImGui is under the cursor — always allow interaction
                        let _ = state.render_ctx.window.set_cursor_hittest(true);
                    } else if lx < state.mascot_alpha_width && ly < state.mascot_alpha_height {
                        let idx = (ly * state.mascot_alpha_width + lx) as usize;
                        let alpha = state.mascot_alpha_map.get(idx).copied().unwrap_or(0);
                        let on_model = alpha > 0;
                        let _ = state.render_ctx.window.set_cursor_hittest(on_model);
                    } else {
                        let _ = state.render_ctx.window.set_cursor_hittest(false);
                    }
                }
                // Note: drag is handled by OS-native drag_window(), no manual update needed.
            }
            WindowEvent::MouseInput {
                state: btn_state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if state.mascot.enabled {
                    // Don't drag the window when ImGui wants the mouse
                    // (e.g. clicking a button, dragging a slider)
                    let imgui_wants = state.show_imgui
                        && state.imgui.as_ref().map_or(false, |im| im.want_capture_mouse());

                    match btn_state {
                        ElementState::Pressed if !imgui_wants => {
                            // Use OS-native window drag to avoid cursor feedback loop.
                            if let Err(e) = state.render_ctx.window.drag_window() {
                                log::warn!("drag_window failed: {e}");
                            }
                        }
                        _ => {
                            // ImGui is handling this click, or button released.
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
        always_on_top: state.mascot.always_on_top,
        fullscreen: state.fullscreen,
    }
    .save();
}
