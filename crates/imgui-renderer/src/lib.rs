//! Dear ImGui integration for wgpu applications.
//!
//! Provides [`ImGuiRenderer`] — a thin wrapper around `imgui`, `imgui-wgpu`,
//! and `imgui-winit-support` that can be integrated into any wgpu + winit
//! application in three method calls: [`handle_event`](ImGuiRenderer::handle_event),
//! [`frame`](ImGuiRenderer::frame), and [`render`](ImGuiRenderer::render).
//!
//! # Quick Start
//!
//! ```rust,no_run
//! # fn example(
//! #     device: &wgpu::Device,
//! #     queue: &wgpu::Queue,
//! #     window: &winit::window::Window,
//! #     view: &wgpu::TextureView,
//! # ) {
//! use imgui_renderer::ImGuiRenderer;
//!
//! let mut imgui = ImGuiRenderer::new(
//!     device,
//!     queue,
//!     wgpu::TextureFormat::Bgra8UnormSrgb,
//!     window,
//! ).unwrap();
//!
//! // In your event handler:
//! // imgui.handle_event(window, &event);
//!
//! // Each frame:
//! imgui.frame(window, |ui| {
//!     ui.show_demo_window(&mut true);
//! });
//! imgui.render(device, queue, view);
//! # }
//! ```

pub use imgui;
pub use imgui_wgpu;
pub use imgui_winit_support;

use imgui::{Context, FontConfig, FontSource, MouseCursor};
use imgui_wgpu::{Renderer, RendererConfig};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::time::{Duration, Instant};
use winit::event::{Event, WindowEvent};
use winit::window::Window;

/// Dear ImGui renderer integrated with wgpu and winit.
///
/// Wraps an imgui [`Context`], a [`WinitPlatform`] backend, and an
/// imgui-wgpu [`Renderer`] into a single type with a simple three-method API.
pub struct ImGuiRenderer {
    ctx: Context,
    platform: WinitPlatform,
    renderer: Renderer,
    last_frame: Instant,
    last_cursor: Option<MouseCursor>,
}

impl ImGuiRenderer {
    /// Create a new ImGui renderer.
    ///
    /// # Arguments
    /// * `device`  — wgpu device
    /// * `queue`   — wgpu queue
    /// * `format`  — surface texture format (e.g. `Bgra8UnormSrgb`)
    /// * `window`  — winit window to attach to
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        window: &Window,
    ) -> anyhow::Result<Self> {
        let mut ctx = Context::create();
        ctx.set_ini_filename(None);

        // Enable docking
        ctx.io_mut().config_flags |= imgui::ConfigFlags::DOCKING_ENABLE;

        let mut platform = WinitPlatform::new(&mut ctx);
        platform.attach_window(ctx.io_mut(), window, HiDpiMode::Default);

        // Configure fonts for HiDPI — smaller size for compact UI
        let hidpi_factor = window.scale_factor();
        let font_size = (10.0 * hidpi_factor) as f32;
        ctx.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        ctx.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig {
                size_pixels: font_size,
                ..Default::default()
            }),
        }]);

        // Dark gray style
        let style = ctx.style_mut();
        style.window_rounding = 4.0;
        style.frame_rounding = 2.0;
        style.grab_rounding = 2.0;
        style.window_border_size = 0.0;
        // Colors: dark gray theme
        style.colors[imgui::sys::ImGuiCol_WindowBg as usize] = [0.12, 0.12, 0.14, 0.90];
        style.colors[imgui::sys::ImGuiCol_TitleBg as usize] = [0.08, 0.08, 0.10, 1.00];
        style.colors[imgui::sys::ImGuiCol_TitleBgActive as usize] = [0.14, 0.14, 0.18, 1.00];
        style.colors[imgui::sys::ImGuiCol_FrameBg as usize] = [0.18, 0.18, 0.22, 1.00];
        style.colors[imgui::sys::ImGuiCol_FrameBgHovered as usize] = [0.24, 0.24, 0.30, 1.00];
        style.colors[imgui::sys::ImGuiCol_FrameBgActive as usize] = [0.30, 0.30, 0.38, 1.00];
        style.colors[imgui::sys::ImGuiCol_Header as usize] = [0.18, 0.18, 0.22, 1.00];
        style.colors[imgui::sys::ImGuiCol_HeaderHovered as usize] = [0.24, 0.24, 0.30, 1.00];
        style.colors[imgui::sys::ImGuiCol_HeaderActive as usize] = [0.30, 0.30, 0.38, 1.00];
        style.colors[imgui::sys::ImGuiCol_Button as usize] = [0.20, 0.20, 0.26, 1.00];
        style.colors[imgui::sys::ImGuiCol_ButtonHovered as usize] = [0.28, 0.28, 0.36, 1.00];
        style.colors[imgui::sys::ImGuiCol_ButtonActive as usize] = [0.35, 0.35, 0.44, 1.00];
        style.colors[imgui::sys::ImGuiCol_SliderGrab as usize] = [0.40, 0.40, 0.50, 1.00];
        style.colors[imgui::sys::ImGuiCol_SliderGrabActive as usize] = [0.50, 0.50, 0.65, 1.00];
        style.colors[imgui::sys::ImGuiCol_CheckMark as usize] = [0.55, 0.65, 0.85, 1.00];
        style.colors[imgui::sys::ImGuiCol_Text as usize] = [0.85, 0.85, 0.88, 1.00];
        style.colors[imgui::sys::ImGuiCol_Separator as usize] = [0.25, 0.25, 0.30, 1.00];

        let renderer_config = RendererConfig {
            texture_format: format,
            ..Default::default()
        };

        let renderer = Renderer::new(&mut ctx, device, queue, renderer_config);

        Ok(Self {
            ctx,
            platform,
            renderer,
            last_frame: Instant::now(),
            last_cursor: None,
        })
    }

    /// Forward a winit [`WindowEvent`] to ImGui.
    ///
    /// Call this from your `ApplicationHandler::window_event` for every event.
    pub fn handle_event(
        &mut self,
        window: &Window,
        window_id: winit::window::WindowId,
        event: &WindowEvent,
    ) {
        self.platform.handle_event::<()>(
            self.ctx.io_mut(),
            window,
            &Event::WindowEvent {
                window_id,
                event: event.clone(),
            },
        );
    }

    /// Forward a non-window event to ImGui (e.g. `AboutToWait`, `DeviceEvent`).
    ///
    /// Typically called from `about_to_wait` with `Event::AboutToWait`.
    pub fn handle_non_window_event<T: 'static>(&mut self, window: &Window, event: &Event<T>) {
        self.platform.handle_event(self.ctx.io_mut(), window, event);
    }

    /// Build the ImGui frame.
    ///
    /// The closure `f` receives a [`imgui::Ui`] reference to define the UI.
    /// After this call, use [`render`](Self::render) to draw the frame.
    pub fn frame<F: FnOnce(&imgui::Ui)>(&mut self, window: &Window, f: F) {
        let now = Instant::now();
        self.ctx.io_mut().update_delta_time(now - self.last_frame);
        self.last_frame = now;

        self.platform
            .prepare_frame(self.ctx.io_mut(), window)
            .expect("Failed to prepare ImGui frame");

        let ui = self.ctx.frame();
        f(ui);

        self.last_cursor = ui.mouse_cursor();
        self.platform.prepare_render(ui, window);
    }

    /// Build the ImGui frame with an explicit delta time.
    ///
    /// Same as [`frame`](Self::frame) but uses the provided `dt` instead of
    /// measuring elapsed time automatically.
    pub fn frame_with_dt<F: FnOnce(&imgui::Ui)>(&mut self, window: &Window, dt: Duration, f: F) {
        self.ctx.io_mut().update_delta_time(dt);
        self.last_frame = Instant::now();

        self.platform
            .prepare_frame(self.ctx.io_mut(), window)
            .expect("Failed to prepare ImGui frame");

        let ui = self.ctx.frame();
        f(ui);

        self.last_cursor = ui.mouse_cursor();
        self.platform.prepare_render(ui, window);
    }

    /// Render the ImGui draw data onto the given texture view.
    ///
    /// Uses `LoadOp::Load` so that ImGui is drawn on top of whatever was
    /// already rendered (overlay mode).
    /// Notify ImGui that the window/surface was resized.
    /// Call this whenever the wgpu surface is reconfigured.
    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f64) {
        let io = self.ctx.io_mut();
        let logical_w = width as f64 / scale_factor;
        let logical_h = height as f64 / scale_factor;
        io.display_size = [logical_w as f32, logical_h as f32];
        io.display_framebuffer_scale = [scale_factor as f32, scale_factor as f32];
    }

    pub fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, view: &wgpu::TextureView) {
        let draw_data = self.ctx.render();
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("imgui_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            self.renderer
                .render(draw_data, queue, device, &mut rpass)
                .expect("ImGui rendering failed");
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render ImGui draw data into an existing render pass.
    ///
    /// Use this when you want to control the render pass yourself rather than
    /// having [`render`](Self::render) create one.
    pub fn render_into_pass<'a>(
        &'a mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        rpass: &mut wgpu::RenderPass<'a>,
    ) {
        let draw_data = self.ctx.render();
        self.renderer
            .render(draw_data, queue, device, rpass)
            .expect("ImGui rendering failed");
    }

    /// Access the underlying imgui [`Context`].
    pub fn context(&self) -> &Context {
        &self.ctx
    }

    /// Mutably access the underlying imgui [`Context`].
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.ctx
    }

    /// Access the underlying imgui-wgpu [`Renderer`].
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    /// Mutably access the underlying imgui-wgpu [`Renderer`].
    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    /// Reload the font texture after font changes.
    pub fn reload_font_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.renderer
            .reload_font_texture(&mut self.ctx, device, queue);
    }

    /// Returns true if ImGui wants to capture mouse input.
    pub fn want_capture_mouse(&self) -> bool {
        self.ctx.io().want_capture_mouse
    }

    /// Returns true if ImGui wants to capture keyboard input.
    pub fn want_capture_keyboard(&self) -> bool {
        self.ctx.io().want_capture_keyboard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that ImGuiRenderer's API compiles with the expected types.
    /// This is a compile-time check only; actual rendering requires a GPU.
    #[test]
    fn api_signature_check() {
        fn _check_new(_device: &wgpu::Device, _queue: &wgpu::Queue, _window: &Window) {
            // This function only needs to compile, not run.
            // ImGuiRenderer::new(device, queue, wgpu::TextureFormat::Bgra8UnormSrgb, window)
        }
    }
}
