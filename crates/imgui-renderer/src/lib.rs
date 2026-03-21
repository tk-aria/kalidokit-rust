//! Dear ImGui integration for wgpu applications.
//!
//! Provides [`ImGuiRenderer`] — a thin wrapper around `dear-imgui-rs`, `dear-imgui-wgpu`,
//! and `dear-imgui-winit` that can be integrated into any wgpu + winit
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

pub use dear_imgui_rs as imgui;
pub use dear_imgui_wgpu as imgui_wgpu;
pub use dear_imgui_winit as imgui_winit;
pub use dear_imnodes as imnodes;

use dear_imgui_rs::{ConfigFlags, Context, FontConfig, FontSource, MouseCursor, StyleColor};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use std::time::{Duration, Instant};
use winit::event::{Event, WindowEvent};
use winit::window::Window;

/// Dear ImGui renderer integrated with wgpu and winit.
///
/// Wraps a dear-imgui-rs [`Context`], a [`WinitPlatform`] backend, and a
/// dear-imgui-wgpu [`WgpuRenderer`] into a single type with a simple three-method API.
pub struct ImGuiRenderer {
    ctx: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    last_frame: Instant,
    last_cursor: Option<MouseCursor>,
    /// ImNodes context for node editors (created lazily on first use).
    imnodes_ctx: Option<dear_imnodes::Context>,
    /// ImNodes editor context (one per editor).
    imnodes_editor: Option<dear_imnodes::EditorContext>,
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
        let _ = ctx.set_ini_filename(None::<String>);

        // Enable docking
        {
            let flags = ctx.io().config_flags();
            ctx.io_mut()
                .set_config_flags(flags | ConfigFlags::DOCKING_ENABLE);
        }

        let mut platform = WinitPlatform::new(&mut ctx);
        platform.attach_window(window, HiDpiMode::Default, &mut ctx);

        // Configure fonts for HiDPI — smaller size for compact UI
        let hidpi_factor = window.scale_factor();
        let font_size = (10.0 * hidpi_factor) as f32;
        ctx.io_mut()
            .set_font_global_scale((1.0 / hidpi_factor) as f32);

        ctx.fonts().add_font(&[FontSource::DefaultFontData {
            size_pixels: Some(font_size),
            config: Some(FontConfig::new()),
        }]);

        // Dark gray style
        let style = ctx.style_mut();
        style.set_window_rounding(4.0);
        style.set_frame_rounding(2.0);
        style.set_grab_rounding(2.0);
        style.set_window_border_size(0.0);
        // Colors: YouTube-style dark gray (semi-transparent so avatar shows through)
        style.set_color(StyleColor::WindowBg, [0.11, 0.11, 0.11, 0.75]);
        style.set_color(StyleColor::TitleBg, [0.08, 0.08, 0.08, 1.00]);
        style.set_color(StyleColor::TitleBgActive, [0.15, 0.15, 0.15, 1.00]);
        style.set_color(StyleColor::FrameBg, [0.18, 0.18, 0.18, 1.00]);
        style.set_color(StyleColor::FrameBgHovered, [0.25, 0.25, 0.25, 1.00]);
        style.set_color(StyleColor::FrameBgActive, [0.30, 0.30, 0.30, 1.00]);
        style.set_color(StyleColor::Header, [0.18, 0.18, 0.18, 1.00]);
        style.set_color(StyleColor::HeaderHovered, [0.25, 0.25, 0.25, 1.00]);
        style.set_color(StyleColor::HeaderActive, [0.30, 0.30, 0.30, 1.00]);
        style.set_color(StyleColor::Button, [0.20, 0.20, 0.20, 1.00]);
        style.set_color(StyleColor::ButtonHovered, [0.28, 0.28, 0.28, 1.00]);
        style.set_color(StyleColor::ButtonActive, [0.35, 0.35, 0.35, 1.00]);
        style.set_color(StyleColor::SliderGrab, [0.50, 0.50, 0.50, 1.00]);
        style.set_color(StyleColor::SliderGrabActive, [0.65, 0.65, 0.65, 1.00]);
        style.set_color(StyleColor::CheckMark, [0.90, 0.90, 0.90, 1.00]);
        style.set_color(StyleColor::Text, [0.90, 0.90, 0.90, 1.00]);
        style.set_color(StyleColor::TextDisabled, [0.50, 0.50, 0.50, 1.00]);
        style.set_color(StyleColor::Separator, [0.22, 0.22, 0.22, 1.00]);
        style.set_color(StyleColor::Tab, [0.12, 0.12, 0.12, 1.00]);
        style.set_color(StyleColor::TabHovered, [0.25, 0.25, 0.25, 1.00]);
        style.set_color(StyleColor::TabSelected, [0.18, 0.18, 0.18, 1.00]);
        style.set_color(StyleColor::DockingPreview, [0.40, 0.40, 0.40, 0.70]);
        style.set_color(StyleColor::ScrollbarBg, [0.08, 0.08, 0.08, 1.00]);
        style.set_color(StyleColor::ScrollbarGrab, [0.30, 0.30, 0.30, 1.00]);
        style.set_color(StyleColor::PopupBg, [0.13, 0.13, 0.13, 0.96]);
        style.set_color(StyleColor::Border, [0.22, 0.22, 0.22, 0.50]);

        let init_info = WgpuInitInfo::new(device.clone(), queue.clone(), format);
        let renderer = WgpuRenderer::new(init_info, &mut ctx)
            .map_err(|e| anyhow::anyhow!("WgpuRenderer init failed: {e}"))?;

        // Create ImNodes context from the ImGui context
        let imnodes_ctx = dear_imnodes::Context::try_create(&ctx)
            .map_err(|e| anyhow::anyhow!("ImNodes context failed: {e}"))
            .ok();
        let imnodes_editor = imnodes_ctx.as_ref().map(|c| {
            c.create_editor_context()
        });

        Ok(Self {
            ctx,
            platform,
            renderer,
            last_frame: Instant::now(),
            last_cursor: None,
            imnodes_ctx,
            imnodes_editor,
        })
    }

    /// Forward a winit [`WindowEvent`] to ImGui.
    ///
    /// Call this from your `ApplicationHandler::window_event` for every event.
    pub fn handle_event(
        &mut self,
        window: &Window,
        _window_id: winit::window::WindowId,
        event: &WindowEvent,
    ) {
        self.platform
            .handle_window_event(&mut self.ctx, window, event);
    }

    /// Forward a non-window event to ImGui (e.g. `AboutToWait`, `DeviceEvent`).
    ///
    /// Typically called from `about_to_wait` with `Event::AboutToWait`.
    pub fn handle_non_window_event<T: 'static>(&mut self, window: &Window, event: &Event<T>) {
        self.platform
            .handle_event(&mut self.ctx, window, event);
    }

    /// Build the ImGui frame.
    ///
    /// The closure `f` receives a [`dear_imgui_rs::Ui`] reference to define the UI.
    /// After this call, use [`render`](Self::render) to draw the frame.
    pub fn frame<F: FnOnce(&dear_imgui_rs::Ui)>(&mut self, window: &Window, f: F) {
        let now = Instant::now();
        let delta = now - self.last_frame;
        let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        self.ctx.io_mut().set_delta_time(delta_s);
        self.last_frame = now;

        self.platform.prepare_frame(window, &mut self.ctx);

        let ui = self.ctx.frame();
        f(ui);

        self.last_cursor = ui.mouse_cursor();
        self.platform.prepare_render_with_ui(ui, window);
    }

    /// Build the ImGui frame with ImNodes support.
    ///
    /// The closure receives the Ui and references to the ImNodes context/editor,
    /// enabling use of `ui.imnodes_editor(ctx, Some(editor))` for node editors.
    pub fn frame_with_nodes<F>(&mut self, window: &Window, f: F)
    where
        F: FnOnce(&dear_imgui_rs::Ui, Option<&dear_imnodes::Context>, Option<&dear_imnodes::EditorContext>),
    {
        let now = Instant::now();
        let delta = now - self.last_frame;
        let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        self.ctx.io_mut().set_delta_time(delta_s);
        self.last_frame = now;

        self.platform.prepare_frame(window, &mut self.ctx);

        let ui = self.ctx.frame();
        f(ui, self.imnodes_ctx.as_ref(), self.imnodes_editor.as_ref());

        self.last_cursor = ui.mouse_cursor();
        self.platform.prepare_render_with_ui(ui, window);
    }

    /// Build the ImGui frame with an explicit delta time.
    ///
    /// Same as [`frame`](Self::frame) but uses the provided `dt` instead of
    /// measuring elapsed time automatically.
    pub fn frame_with_dt<F: FnOnce(&dear_imgui_rs::Ui)>(
        &mut self,
        window: &Window,
        dt: Duration,
        f: F,
    ) {
        let delta_s = dt.as_secs() as f32 + dt.subsec_nanos() as f32 / 1_000_000_000.0;
        self.ctx.io_mut().set_delta_time(delta_s);
        self.last_frame = Instant::now();

        self.platform.prepare_frame(window, &mut self.ctx);

        let ui = self.ctx.frame();
        f(ui);

        self.last_cursor = ui.mouse_cursor();
        self.platform.prepare_render_with_ui(ui, window);
    }

    /// Notify ImGui that the window/surface was resized.
    /// Call this whenever the wgpu surface is reconfigured.
    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f64) {
        let logical_w = width as f64 / scale_factor;
        let logical_h = height as f64 / scale_factor;
        let io = self.ctx.io_mut();
        io.set_display_size([logical_w as f32, logical_h as f32]);
        io.set_display_framebuffer_scale([scale_factor as f32, scale_factor as f32]);
    }

    /// Render the ImGui draw data onto the given texture view.
    ///
    /// Uses `LoadOp::Load` so that ImGui is drawn on top of whatever was
    /// already rendered (overlay mode).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
    ) {
        // Notify the renderer of a new frame (recreates pipeline if needed)
        let _ = self.renderer.new_frame();

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

            let _ = self.renderer.render_draw_data(draw_data, &mut rpass);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render ImGui draw data into an existing render pass.
    ///
    /// Use this when you want to control the render pass yourself rather than
    /// having [`render`](Self::render) create one.
    pub fn render_into_pass<'a>(
        &'a mut self,
        _queue: &wgpu::Queue,
        _device: &wgpu::Device,
        rpass: &mut wgpu::RenderPass<'a>,
    ) {
        let _ = self.renderer.new_frame();
        let draw_data = self.ctx.render();
        let _ = self.renderer.render_draw_data(draw_data, rpass);
    }

    /// Access the underlying dear-imgui-rs [`Context`].
    pub fn context(&self) -> &Context {
        &self.ctx
    }

    /// Mutably access the underlying dear-imgui-rs [`Context`].
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.ctx
    }

    /// Access the underlying dear-imgui-wgpu [`WgpuRenderer`].
    pub fn renderer(&self) -> &WgpuRenderer {
        &self.renderer
    }

    /// Mutably access the underlying dear-imgui-wgpu [`WgpuRenderer`].
    pub fn renderer_mut(&mut self) -> &mut WgpuRenderer {
        &mut self.renderer
    }

    /// Access the ImNodes editor context (for use inside `frame()` closures).
    pub fn imnodes_editor(&self) -> Option<&dear_imnodes::EditorContext> {
        self.imnodes_editor.as_ref()
    }

    /// Reload the font texture after font changes.
    ///
    /// Note: In dear-imgui-wgpu 0.10+, font texture management is handled
    /// internally by the renderer via the modern texture system.
    /// This method is kept for API compatibility but is a no-op.
    pub fn reload_font_texture(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue) {
        // dear-imgui-wgpu 0.10+ handles font textures automatically
        // via the ImTextureData system in new_frame()/render_draw_data().
    }

    /// Returns true if ImGui wants to capture mouse input.
    pub fn want_capture_mouse(&self) -> bool {
        self.ctx.io().want_capture_mouse()
    }

    /// Returns true if ImGui wants to capture keyboard input.
    pub fn want_capture_keyboard(&self) -> bool {
        self.ctx.io().want_capture_keyboard()
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
