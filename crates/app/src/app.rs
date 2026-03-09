use std::sync::Arc;

use renderer::camera::Camera;
use renderer::context::RenderContext;
use renderer::pipeline::create_render_pipeline;
use renderer::vertex::Vertex;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

// Triangle vertices (front-facing, CCW)
const TRIANGLE_VERTICES: &[Vertex] = &[
    Vertex {
        position: [0.0, 0.5, 0.0],
        normal: [0.0, 0.0, -1.0],
        uv: [0.5, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.0],
        normal: [0.0, 0.0, -1.0],
        uv: [0.0, 1.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0],
        normal: [0.0, 0.0, -1.0],
        uv: [1.0, 1.0],
    },
];

const TRIANGLE_INDICES: &[u32] = &[0, 1, 2];

struct GpuState {
    ctx: RenderContext,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera: Camera,
}

pub struct App {
    state: Option<GpuState>,
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

        let ctx = pollster::block_on(RenderContext::new(window)).unwrap();

        // Camera bind group layout
        let camera_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("camera_bind_group_layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let shader_src = include_str!("../../../assets/shaders/basic.wgsl");
        let pipeline = create_render_pipeline(
            &ctx.device,
            ctx.config.format,
            shader_src,
            &[&camera_bind_group_layout],
            None,
        );

        // Vertex / index buffers
        use wgpu::util::DeviceExt;
        let vertex_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex_buffer"),
                contents: bytemuck::cast_slice(TRIANGLE_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("index_buffer"),
                contents: bytemuck::cast_slice(TRIANGLE_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        // Camera
        let camera = Camera {
            aspect: ctx.config.width as f32 / ctx.config.height as f32,
            ..Camera::default()
        };
        let camera_uniform = camera.to_uniform(glam::Mat4::IDENTITY);
        let camera_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("camera_buffer"),
                contents: bytemuck::bytes_of(&camera_uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let camera_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        self.state = Some(GpuState {
            ctx,
            pipeline,
            vertex_buffer,
            index_buffer,
            camera_buffer,
            camera_bind_group,
            camera,
        });
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
                state.ctx.resize(size.width, size.height);
                state.camera.aspect = size.width as f32 / size.height.max(1) as f32;
            }
            WindowEvent::RedrawRequested => {
                // Update camera uniform
                let camera_uniform = state.camera.to_uniform(glam::Mat4::IDENTITY);
                state.ctx.queue.write_buffer(
                    &state.camera_buffer,
                    0,
                    bytemuck::bytes_of(&camera_uniform),
                );

                // Render
                let output = match state.ctx.surface.get_current_texture() {
                    Ok(t) => t,
                    Err(_) => return,
                };
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state
                    .ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("render_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.5,
                                    b: 0.2,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        ..Default::default()
                    });
                    pass.set_pipeline(&state.pipeline);
                    pass.set_bind_group(0, &state.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, state.vertex_buffer.slice(..));
                    pass.set_index_buffer(
                        state.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    pass.draw_indexed(0..TRIANGLE_INDICES.len() as u32, 0, 0..1);
                }
                state.ctx.queue.submit(std::iter::once(encoder.finish()));
                output.present();
                state.ctx.window.request_redraw();
            }
            _ => {}
        }
    }
}
