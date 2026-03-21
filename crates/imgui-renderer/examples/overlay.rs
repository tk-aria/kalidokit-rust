//! Overlay example: colored background with ImGui overlay.
//!
//! Demonstrates that ImGui renders on top of existing content using
//! `LoadOp::Load` (overlay mode).
//!
//! Run with: `cargo run -p imgui-renderer --example overlay`

use imgui_renderer::ImGuiRenderer;
use pollster::block_on;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::Window,
};

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
}

struct AppState {
    window: Arc<Window>,
    gpu: GpuState,
    imgui: ImGuiRenderer,
    bg_color: [f32; 3],
    overlay_alpha: f32,
}

#[derive(Default)]
struct App {
    state: Option<AppState>,
}

impl App {
    fn init(&mut self, event_loop: &ActiveEventLoop) {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::default()
        });

        let size = LogicalSize::new(1024.0, 600.0);
        let attrs = Window::default_attributes()
            .with_inner_size(size)
            .with_title("imgui-renderer overlay example");
        let window = Arc::new(event_loop.create_window(attrs).unwrap());

        let inner_size = window.inner_size();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: inner_size.width,
            height: inner_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };
        surface.configure(&device, &surface_config);

        let imgui = ImGuiRenderer::new(&device, &queue, surface_config.format, &window)
            .expect("Failed to create ImGuiRenderer");

        self.state = Some(AppState {
            window,
            gpu: GpuState {
                device,
                queue,
                surface,
                surface_config,
            },
            imgui,
            bg_color: [0.0, 0.3, 0.6],
            overlay_alpha: 0.85,
        });
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            self.init(event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = self.state.as_mut().unwrap();
        state.imgui.handle_event(&state.window, window_id, &event);

        match &event {
            WindowEvent::Resized(size) => {
                state.gpu.surface_config.width = size.width;
                state.gpu.surface_config.height = size.height;
                state
                    .gpu
                    .surface
                    .configure(&state.gpu.device, &state.gpu.surface_config);
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let Key::Named(NamedKey::Escape) = event.logical_key {
                    if event.state.is_pressed() {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let frame = match state.gpu.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Timeout) => return,
                    Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                        state
                            .gpu
                            .surface
                            .configure(&state.gpu.device, &state.gpu.surface_config);
                        return;
                    }
                    Err(e) => {
                        eprintln!("get_current_texture error: {e:?}");
                        return;
                    }
                };

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                // Step 1: Clear with background color (simulates a 3D scene)
                let bg = &state.bg_color;
                let mut encoder = state
                    .gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("background_clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: bg[0] as f64,
                                    g: bg[1] as f64,
                                    b: bg[2] as f64,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                }
                state.gpu.queue.submit(std::iter::once(encoder.finish()));

                // Step 2: ImGui overlay on top of the background
                let bg_color = &mut state.bg_color;
                let overlay_alpha = &mut state.overlay_alpha;
                state.imgui.frame(&state.window, |ui| {
                    ui.window("Overlay Controls")
                        .size(
                            [350.0, 200.0],
                            imgui_renderer::imgui::Condition::FirstUseEver,
                        )
                        .position([50.0, 50.0], imgui_renderer::imgui::Condition::FirstUseEver)
                        .bg_alpha(*overlay_alpha)
                        .build(|| {
                            ui.text("This ImGui panel is overlaid on a colored background.");
                            ui.separator();

                            ui.color_edit3("Background Color", bg_color);
                            ui.slider("Panel Alpha", 0.1, 1.0, overlay_alpha);

                            ui.separator();
                            ui.text_colored(
                                [0.5, 1.0, 0.5, 1.0],
                                "The background color changes in real-time!",
                            );
                        });

                    ui.window("Stats")
                        .size(
                            [250.0, 100.0],
                            imgui_renderer::imgui::Condition::FirstUseEver,
                        )
                        .position(
                            [450.0, 50.0],
                            imgui_renderer::imgui::Condition::FirstUseEver,
                        )
                        .build(|| {
                            let mouse_pos = ui.io().mouse_pos();
                            ui.text(format!("Mouse: ({:.0}, {:.0})", mouse_pos[0], mouse_pos[1]));
                            ui.text(format!(
                                "BG: ({:.2}, {:.2}, {:.2})",
                                bg_color[0], bg_color[1], bg_color[2]
                            ));
                        });
                });

                state
                    .imgui
                    .render(&state.gpu.device, &state.gpu.queue, &view);

                frame.present();
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            state.window.request_redraw();
            state
                .imgui
                .handle_non_window_event(&state.window, &winit::event::Event::<()>::AboutToWait);
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::default()).unwrap();
}
