//! Standalone ImGui demo window example.
//!
//! Opens a winit window with wgpu, initialises ImGuiRenderer, and shows the
//! ImGui Demo Window plus a small info panel with FPS.
//!
//! Run with: `cargo run -p imgui-renderer --example standalone`

use imgui_renderer::ImGuiRenderer;
use pollster::block_on;
use std::sync::Arc;
use std::time::Instant;
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
    show_demo: bool,
    frame_count: u64,
    fps_last_update: Instant,
    fps: f64,
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

        let size = LogicalSize::new(1280.0, 720.0);
        let attrs = Window::default_attributes()
            .with_inner_size(size)
            .with_title("imgui-renderer standalone example");
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
            show_demo: true,
            frame_count: 0,
            fps_last_update: Instant::now(),
            fps: 0.0,
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

        // Forward events to ImGui
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
                // FPS calculation
                state.frame_count += 1;
                let elapsed = state.fps_last_update.elapsed().as_secs_f64();
                if elapsed >= 1.0 {
                    state.fps = state.frame_count as f64 / elapsed;
                    state.frame_count = 0;
                    state.fps_last_update = Instant::now();
                }

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

                // Clear background
                let mut encoder = state
                    .gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clear_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.2,
                                    b: 0.3,
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

                // ImGui frame
                let show_demo = &mut state.show_demo;
                let fps = state.fps;
                state.imgui.frame(&state.window, |ui| {
                    if *show_demo {
                        ui.show_demo_window(show_demo);
                    }

                    ui.window("Info")
                        .size(
                            [300.0, 100.0],
                            imgui_renderer::imgui::Condition::FirstUseEver,
                        )
                        .build(|| {
                            ui.text(format!("FPS: {fps:.0}"));
                            ui.separator();
                            let mouse_pos = ui.io().mouse_pos();
                            ui.text(format!("Mouse: ({:.0}, {:.0})", mouse_pos[0], mouse_pos[1]));
                            ui.checkbox("Show Demo Window", show_demo);
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
