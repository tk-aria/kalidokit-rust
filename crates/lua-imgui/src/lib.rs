//! Lua scripting for Dear ImGui over wgpu.
//!
//! Provides `LuaImgui` which embeds a Lua 5.4 runtime, registers imgui
//! API bindings, and renders Dear ImGui draw data via wgpu.

pub mod bindings;
pub mod commands;
pub mod events;
pub mod renderer;

use std::sync::Arc;

use anyhow::Result;
use lua_runtime::LuaRuntime;
use winit::window::Window;

/// Lua-driven Dear ImGui overlay rendered via wgpu.
pub struct LuaImgui {
    lua: LuaRuntime,
    imgui: imgui::Context,
    renderer: renderer::ImguiRenderer,
    commands: Arc<std::sync::Mutex<Vec<commands::ImguiCommand>>>,
    last_frame_time: std::time::Instant,
}

impl LuaImgui {
    /// Create a new Lua-ImGui instance.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        window: &Window,
    ) -> Result<Self> {
        let lua = LuaRuntime::new()?;
        let mut imgui = imgui::Context::create();

        // Configure imgui
        imgui.set_ini_filename(None);
        let io = imgui.io_mut();
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        io.display_size = [size.width as f32 / scale, size.height as f32 / scale];
        io.display_framebuffer_scale = [scale, scale];
        io.font_global_scale = 1.0;

        // Build font atlas
        let fonts = imgui.fonts();
        fonts.add_font(&[imgui::FontSource::DefaultFontData {
            config: Some(imgui::FontConfig {
                size_pixels: 14.0 * scale,
                ..Default::default()
            }),
        }]);
        let font_tex = fonts.build_rgba32_texture();

        // Create renderer
        let renderer =
            renderer::ImguiRenderer::new(device, queue, surface_format, &font_tex)?;

        // Register Lua bindings
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        bindings::register(&lua, commands.clone())?;

        Ok(Self {
            lua,
            imgui,
            renderer,
            commands,
            last_frame_time: std::time::Instant::now(),
        })
    }

    /// Load a Lua UI script.
    pub fn load_script(&self, path: &std::path::Path) -> Result<()> {
        self.lua.exec_file(path)
    }

    /// Access the Lua runtime for custom bindings.
    pub fn lua(&self) -> &LuaRuntime {
        &self.lua
    }

    /// Forward a winit event to imgui. Returns true if imgui wants to capture it.
    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        events::handle_event(&mut self.imgui, event)
    }

    /// Update display size on window resize.
    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f64) {
        let scale = scale_factor as f32;
        let io = self.imgui.io_mut();
        io.display_size = [width as f32 / scale, height as f32 / scale];
        io.display_framebuffer_scale = [scale, scale];
    }

    /// Run one imgui frame: call Lua update(), collect commands, render.
    ///
    /// Creates its own command encoder and submits to the queue.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
    ) -> Result<()> {
        // Update delta time
        let now = std::time::Instant::now();
        self.imgui.io_mut().delta_time = (now - self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Clear command buffer
        self.commands.lock().unwrap().clear();

        // Call Lua update(dt)
        let dt = self.imgui.io().delta_time;
        if let Err(e) = self.lua.call_global::<_, ()>("update", dt) {
            // Only warn if function exists but errored (not if missing)
            let msg = format!("{e}");
            if !msg.contains("not found") && !msg.contains("is not a function") {
                log::warn!("Lua update error: {e}");
            }
        }

        // Build imgui frame from commands
        let ui = self.imgui.new_frame();
        {
            let cmds = self.commands.lock().unwrap();
            commands::replay_nested(&cmds, ui);
        }

        // Render
        let draw_data = self.imgui.render();
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.renderer
            .render(device, queue, view, &mut encoder, draw_data)?;
        queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }
}
