//! Minimal Dear ImGui wgpu renderer.
//!
//! Renders imgui DrawData using a single wgpu render pipeline with
//! vertex position+UV+color and a font atlas texture.

use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Vertex layout matching imgui's ImDrawVert.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ImDrawVert {
    pos: [f32; 2],
    uv: [f32; 2],
    col: [u8; 4], // RGBA8
}

pub struct ImguiRenderer {
    pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    font_texture: wgpu::Texture,
    font_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl ImguiRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        font_tex: &imgui::FontAtlasTexture,
    ) -> Result<Self> {
        // Font atlas texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("imgui_font_atlas"),
            size: wgpu::Extent3d {
                width: font_tex.width,
                height: font_tex.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            font_tex.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * font_tex.width),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: font_tex.width,
                height: font_tex.height,
                depth_or_array_layers: 1,
            },
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let tex_view = texture.create_view(&Default::default());

        // Texture bind group layout
        let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imgui_tex_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        let font_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("imgui_font_bg"),
            layout: &tex_bgl,
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

        // Uniform buffer (orthographic projection matrix)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("imgui_uniform"),
            size: 64, // mat4x4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imgui_uniform_bgl"),
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

        let uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("imgui_uniform_bg"),
            layout: &uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("imgui_shader"),
            source: wgpu::ShaderSource::Wgsl(IMGUI_WGSL.into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("imgui_pipeline_layout"),
            bind_group_layouts: &[&uniform_bgl, &tex_bgl],
            push_constant_ranges: &[],
        });

        // Render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("imgui_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ImDrawVert>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // pos
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        // uv
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        // col (packed u32 → unorm8x4)
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 16,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            pipeline,
            font_texture: texture,
            font_bind_group: font_bg,
            bind_group_layout: tex_bgl,
            uniform_buffer,
            uniform_bind_group: uniform_bg,
        })
    }

    /// Render imgui draw data.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        draw_data: &imgui::DrawData,
    ) -> Result<()> {
        if draw_data.total_vtx_count == 0 {
            return Ok(());
        }

        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
        if fb_width <= 0.0 || fb_height <= 0.0 {
            return Ok(());
        }

        // Update projection matrix
        let l = draw_data.display_pos[0];
        let r = l + draw_data.display_size[0];
        let t = draw_data.display_pos[1];
        let b = t + draw_data.display_size[1];
        #[rustfmt::skip]
        let proj: [[f32; 4]; 4] = [
            [2.0 / (r - l),     0.0,                0.0, 0.0],
            [0.0,               2.0 / (t - b),      0.0, 0.0],
            [0.0,               0.0,               -1.0, 0.0],
            [(r + l) / (l - r), (t + b) / (b - t),  0.0, 1.0],
        ];
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&proj));

        // Collect all vertices and indices
        let mut vertices = Vec::with_capacity(draw_data.total_vtx_count as usize);
        let mut indices = Vec::with_capacity(draw_data.total_idx_count as usize);

        for draw_list in draw_data.draw_lists() {
            let vtx_buffer = draw_list.vtx_buffer();
            for v in vtx_buffer {
                vertices.push(ImDrawVert {
                    pos: v.pos,
                    uv: v.uv,
                    col: v.col, // [u8; 4] RGBA
                });
            }
            indices.extend_from_slice(draw_list.idx_buffer());
        }

        if vertices.is_empty() {
            return Ok(());
        }

        // Create GPU buffers
        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("imgui_vb"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("imgui_ib"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Render pass
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("imgui_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // preserve existing content
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        pass.set_bind_group(1, &self.font_bind_group, &[]);
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);

        let clip_off = draw_data.display_pos;
        let clip_scale = draw_data.framebuffer_scale;
        let mut vtx_offset = 0u32;
        let mut idx_offset = 0u32;

        for draw_list in draw_data.draw_lists() {
            for cmd in draw_list.commands() {
                match cmd {
                    imgui::DrawCmd::Elements { count, cmd_params } => {
                        let clip = [
                            (cmd_params.clip_rect[0] - clip_off[0]) * clip_scale[0],
                            (cmd_params.clip_rect[1] - clip_off[1]) * clip_scale[1],
                            (cmd_params.clip_rect[2] - clip_off[0]) * clip_scale[0],
                            (cmd_params.clip_rect[3] - clip_off[1]) * clip_scale[1],
                        ];

                        if clip[0] < fb_width
                            && clip[1] < fb_height
                            && clip[2] >= 0.0
                            && clip[3] >= 0.0
                        {
                            let x = clip[0].max(0.0) as u32;
                            let y = clip[1].max(0.0) as u32;
                            let w = (clip[2] - clip[0]).min(fb_width) as u32;
                            let h = (clip[3] - clip[1]).min(fb_height) as u32;
                            if w > 0 && h > 0 {
                                pass.set_scissor_rect(x, y, w, h);
                                let start = idx_offset + cmd_params.idx_offset as u32;
                                let end = start + count as u32;
                                let base = (vtx_offset + cmd_params.vtx_offset as u32) as i32;
                                pass.draw_indexed(start..end, base, 0..1);
                            }
                        }
                    }
                    imgui::DrawCmd::ResetRenderState => {
                        pass.set_pipeline(&self.pipeline);
                    }
                    imgui::DrawCmd::RawCallback { .. } => {}
                }
            }
            vtx_offset += draw_list.vtx_buffer().len() as u32;
            idx_offset += draw_list.idx_buffer().len() as u32;
        }

        Ok(())
    }
}

/// Minimal WGSL shader for imgui rendering.
const IMGUI_WGSL: &str = r#"
struct Uniforms {
    proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) col: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) col: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = u.proj * vec4<f32>(in.pos, 0.0, 1.0);
    out.uv = in.uv;
    out.col = in.col;
    return out;
}

@group(1) @binding(0) var t_texture: texture_2d<f32>;
@group(1) @binding(1) var t_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_col = textureSample(t_texture, t_sampler, in.uv);
    return in.col * tex_col;
}
"#;
