use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use std::sync::Arc;

use crate::auto_blink::BlinkMode;
use crate::state::AppState;

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
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
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
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
