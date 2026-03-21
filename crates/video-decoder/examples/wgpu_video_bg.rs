//! Display a video as a fullscreen background in a wgpu window.
//!
//! On macOS, uses VideoToolbox (AVFoundation HW decode).
//! On other platforms, falls back to the software (openh264) decoder.
//!
//! # Usage
//!
//! ```sh
//! cargo run -p video-decoder --example wgpu_video_bg -- <input.mp4>
//! ```

use std::sync::Arc;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: wgpu_video_bg <input.mp4>");
        std::process::exit(1);
    }
    let input = args[1].clone();

    let event_loop = winit::event_loop::EventLoop::new()?;
    let mut app = App::new(input);
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

/// Unified session wrapper that provides `frame_rgba()` regardless of backend.
enum DecoderSession {
    #[cfg(target_os = "macos")]
    Apple(video_decoder::backend::apple::AppleVideoSession),
    Software(video_decoder::backend::software::SwVideoSession),
}

impl DecoderSession {
    fn as_session(&self) -> &dyn video_decoder::VideoSession {
        match self {
            #[cfg(target_os = "macos")]
            DecoderSession::Apple(s) => s,
            DecoderSession::Software(s) => s,
        }
    }

    fn as_session_mut(&mut self) -> &mut dyn video_decoder::VideoSession {
        match self {
            #[cfg(target_os = "macos")]
            DecoderSession::Apple(s) => s,
            DecoderSession::Software(s) => s,
        }
    }

    fn frame_rgba(&self) -> &[u8] {
        match self {
            #[cfg(target_os = "macos")]
            DecoderSession::Apple(s) => s.frame_rgba(),
            DecoderSession::Software(s) => s.frame_rgba(),
        }
    }
}

struct App {
    input: String,
    gpu: Option<GpuState>,
    session: Option<DecoderSession>,
    last_frame: Option<Instant>,
    window: Option<Arc<winit::window::Window>>,
    // FPS counter
    fps_counter: u32,
    fps_timer: Instant,
    render_fps: f64,
    decode_fps: u32,
    decode_count: u32,
}

impl App {
    fn new(input: String) -> Self {
        Self {
            input,
            gpu: None,
            session: None,
            last_frame: None,
            window: None,
            fps_counter: 0,
            fps_timer: Instant::now(),
            render_fps: 0.0,
            decode_fps: 0,
            decode_count: 0,
        }
    }

    fn init_gpu(&mut self, window: Arc<winit::window::Window>) {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })).expect("no adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("no device");

        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("no surface config");
        surface.configure(&device, &config);

        // Create RGBA texture for decoded frames.
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video_frame"),
            size: wgpu::Extent3d {
                width: 640,
                height: 360,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let tex_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("video_bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fullscreen_quad"),
            source: wgpu::ShaderSource::Wgsl(FULLSCREEN_WGSL.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bgl],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("video_bg_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        self.gpu = Some(GpuState {
            device,
            queue,
            surface,
            texture,
            bind_group,
            pipeline,
        });
    }

    fn init_session(&mut self) {
        use video_decoder::handle::NativeHandle;
        use video_decoder::session::{OutputTarget, SessionConfig};
        use video_decoder::types::{ColorSpace, PixelFormat};

        let output = OutputTarget {
            native_handle: NativeHandle::Metal {
                texture: std::ptr::null_mut(),
                device: std::ptr::null_mut(),
            },
            format: PixelFormat::Rgba8Srgb,
            width: 640,
            height: 360,
            color_space: ColorSpace::default(),
        };
        let config = SessionConfig::default();

        // Try VideoToolbox on macOS, fall back to Software.
        let session: DecoderSession = {
            #[cfg(target_os = "macos")]
            {
                match video_decoder::backend::apple::AppleVideoSession::new(
                    &self.input,
                    output,
                    &config,
                ) {
                    Ok(s) => DecoderSession::Apple(s),
                    Err(e) => {
                        eprintln!("VideoToolbox failed ({}), falling back to SW", e);
                        let sw_output = OutputTarget {
                            native_handle: NativeHandle::Wgpu {
                                queue: std::ptr::null(),
                                texture_id: 0,
                            },
                            ..output
                        };
                        DecoderSession::Software(
                            video_decoder::backend::software::SwVideoSession::new(
                                &self.input,
                                sw_output,
                                &config,
                            )
                            .expect("SW decoder failed"),
                        )
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let sw_output = OutputTarget {
                    native_handle: NativeHandle::Wgpu {
                        queue: std::ptr::null(),
                        texture_id: 0,
                    },
                    ..output
                };
                DecoderSession::Software(
                    video_decoder::backend::software::SwVideoSession::new(
                        &self.input,
                        sw_output,
                        &config,
                    )
                    .expect("SW decoder failed"),
                )
            }
        };

        let info = session.as_session().info();
        println!(
            "Video: {}x{}, {:.1} fps, {:.1}s, backend: {:?}",
            info.width,
            info.height,
            info.fps,
            info.duration.as_secs_f64(),
            info.backend,
        );
        self.session = Some(session);
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = winit::window::WindowAttributes::default()
                .with_title("video-decoder: wgpu_video_bg")
                .with_inner_size(winit::dpi::LogicalSize::new(640, 360));
            let window = Arc::new(event_loop.create_window(attrs).unwrap());
            self.window = Some(window.clone());
            self.init_gpu(window);
            self.init_session();
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            winit::event::WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape),
                        state: winit::event::ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                event_loop.exit();
            }
            winit::event::WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = self
                    .last_frame
                    .map(|prev| now.duration_since(prev))
                    .unwrap_or(std::time::Duration::from_millis(16));
                self.last_frame = Some(now);

                // FPS counter: update every second.
                self.fps_counter += 1;
                let elapsed = now.duration_since(self.fps_timer);
                if elapsed >= std::time::Duration::from_secs(1) {
                    self.render_fps = self.fps_counter as f64 / elapsed.as_secs_f64();
                    self.decode_fps = self.decode_count;
                    self.fps_counter = 0;
                    self.decode_count = 0;
                    self.fps_timer = now;

                    if let Some(window) = &self.window {
                        let backend_name = self
                            .session
                            .as_ref()
                            .map(|s| format!("{:?}", s.as_session().info().backend))
                            .unwrap_or_default();
                        window.set_title(&format!(
                            "video-decoder | render: {:.0} fps | decode: {} fps | backend: {}",
                            self.render_fps, self.decode_fps, backend_name,
                        ));
                    }
                }

                // Decode next frame.
                if let Some(session) = &mut self.session {
                    if let Ok(video_decoder::FrameStatus::NewFrame) =
                        session.as_session_mut().decode_frame(dt)
                    {
                        self.decode_count += 1;
                        // Upload RGBA to GPU texture.
                        if let Some(gpu) = &self.gpu {
                            let rgba = session.frame_rgba();
                            let info = session.as_session().info();

                            gpu.queue.write_texture(
                                wgpu::TexelCopyTextureInfo {
                                    texture: &gpu.texture,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d::ZERO,
                                    aspect: wgpu::TextureAspect::All,
                                },
                                rgba,
                                wgpu::TexelCopyBufferLayout {
                                    offset: 0,
                                    bytes_per_row: Some(info.width * 4),
                                    rows_per_image: Some(info.height),
                                },
                                wgpu::Extent3d {
                                    width: info.width,
                                    height: info.height,
                                    depth_or_array_layers: 1,
                                },
                            );
                        }
                    }
                }

                // Render.
                if let Some(gpu) = &self.gpu {
                    let frame = match gpu.surface.get_current_texture() {
                        Ok(tex) => tex,
                        _ => return,
                    };
                    {
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder = gpu
                            .device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                        {
                            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("video_bg_pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                        store: wgpu::StoreOp::Store,
                                    },
                                    depth_slice: None,
                                })],
                                depth_stencil_attachment: None,
                                ..Default::default()
                            });
                            pass.set_pipeline(&gpu.pipeline);
                            pass.set_bind_group(0, &gpu.bind_group, &[]);
                            pass.draw(0..3, 0..1);
                        }
                        gpu.queue.submit(std::iter::once(encoder.finish()));
                        frame.present();
                    }
                }
            }
            _ => {}
        }
    }
}

const FULLSCREEN_WGSL: &str = r#"
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    let x = f32(i32(vi) / 2) * 4.0 - 1.0;
    let y = f32(i32(vi) % 2) * 4.0 - 1.0;
    var out: VertexOutput;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
"#;
