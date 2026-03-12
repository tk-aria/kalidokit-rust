use crate::camera::CameraUniform;
use crate::context::RenderContext;
use crate::depth::DepthTexture;
use crate::mesh::GpuMesh;
use crate::morph::MorphData;
use crate::pipeline::create_render_pipeline;
use crate::skin::SkinData;
use crate::texture::GpuTexture;
use crate::vertex::Vertex;
use glam::Mat4;
use wgpu::util::DeviceExt;

/// Per-mesh material data on the GPU.
struct GpuMaterial {
    bind_group: wgpu::BindGroup,
}

/// Material uniform data matching the shader's MaterialUniform struct.
/// Includes MToon toon shading parameters.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
    shade_color: [f32; 4],
    rim_color: [f32; 4],
    /// Packed: [shade_shift, shade_toony, rim_power, rim_lift]
    mtoon_params: [f32; 4],
}

pub struct Scene {
    meshes: Vec<GpuMesh>,
    materials: Vec<GpuMaterial>,
    skin: SkinData,
    morph: MorphData,
    depth: DepthTexture,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    // Kept alive for potential future dynamic material creation.
    #[allow(dead_code)]
    material_bind_group_layout: wgpu::BindGroupLayout,
}

/// Input data for one mesh's material (base color + MToon params + optional texture image).
pub struct MeshMaterialInput {
    pub base_color: [f32; 4],
    pub shade_color: [f32; 4],
    pub rim_color: [f32; 4],
    pub shade_shift: f32,
    pub shade_toony: f32,
    pub rim_power: f32,
    pub rim_lift: f32,
    pub base_color_texture: Option<image::DynamicImage>,
}

impl Default for MeshMaterialInput {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            shade_color: [0.5, 0.5, 0.5, 1.0],
            rim_color: [0.0, 0.0, 0.0, 1.0],
            shade_shift: -0.1,
            shade_toony: 0.5,
            rim_power: 1.0,
            rim_lift: 0.0,
            base_color_texture: None,
        }
    }
}

impl Scene {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
        vertices_list: &[(&[Vertex], &[u32])],
        mesh_materials: &[MeshMaterialInput],
        max_joints: usize,
        max_morph_targets: usize,
    ) -> Self {
        let meshes: Vec<GpuMesh> = vertices_list
            .iter()
            .map(|(verts, indices)| GpuMesh::from_vertices_indices(device, verts, indices))
            .collect();

        let skin = SkinData::new(device, max_joints.max(1));
        let morph = MorphData::new(device, max_morph_targets.max(1));
        let depth = DepthTexture::new(device, config.width, config.height);

        // Camera uniform
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_buffer"),
            contents: bytemuck::bytes_of(&CameraUniform::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Material bind group layout (shared by all meshes)
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("material_bind_group_layout"),
                entries: &[
                    // MaterialUniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Base color texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Create default white texture as fallback
        let default_texture = GpuTexture::default_white(device, queue);

        // Create per-mesh material bind groups
        let materials: Vec<GpuMaterial> = (0..meshes.len())
            .map(|i| {
                let mat_input = mesh_materials.get(i);
                let defaults = MeshMaterialInput::default();
                let base_color = mat_input
                    .map(|m| m.base_color)
                    .unwrap_or(defaults.base_color);
                let shade_color = mat_input
                    .map(|m| m.shade_color)
                    .unwrap_or(defaults.shade_color);
                let rim_color = mat_input.map(|m| m.rim_color).unwrap_or(defaults.rim_color);
                let shade_shift = mat_input
                    .map(|m| m.shade_shift)
                    .unwrap_or(defaults.shade_shift);
                let shade_toony = mat_input
                    .map(|m| m.shade_toony)
                    .unwrap_or(defaults.shade_toony);
                let rim_power = mat_input.map(|m| m.rim_power).unwrap_or(defaults.rim_power);
                let rim_lift = mat_input.map(|m| m.rim_lift).unwrap_or(defaults.rim_lift);

                let material_uniform = MaterialUniform {
                    base_color,
                    shade_color,
                    rim_color,
                    mtoon_params: [shade_shift, shade_toony, rim_power, rim_lift],
                };
                let material_buffer =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("material_buffer_{i}")),
                        contents: bytemuck::bytes_of(&material_uniform),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

                // Use the mesh's texture if available, otherwise fall back to default white
                let gpu_texture = mat_input
                    .and_then(|m| m.base_color_texture.as_ref())
                    .map(|img| GpuTexture::from_image(device, queue, img));

                let (tex_view, tex_sampler) = match &gpu_texture {
                    Some(t) => (&t.view, &t.sampler),
                    None => (&default_texture.view, &default_texture.sampler),
                };

                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("material_bind_group_{i}")),
                    layout: &material_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: material_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(tex_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(tex_sampler),
                        },
                    ],
                });

                GpuMaterial { bind_group }
            })
            .collect();

        let shader_src = include_str!("../../../assets/shaders/skinning.wgsl");
        let pipeline = create_render_pipeline(
            device,
            config.format,
            shader_src,
            &[
                &camera_bind_group_layout,
                skin.bind_group_layout(),
                morph.bind_group_layout(),
                &material_bind_group_layout,
            ],
            Some(crate::depth::DEPTH_FORMAT),
        );

        Self {
            meshes,
            materials,
            skin,
            morph,
            depth,
            pipeline,
            camera_buffer,
            camera_bind_group,
            material_bind_group_layout,
        }
    }

    pub fn prepare(
        &self,
        queue: &wgpu::Queue,
        joint_matrices: &[Mat4],
        morph_weights: &[f32],
        camera_uniform: &CameraUniform,
    ) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(camera_uniform));
        if !joint_matrices.is_empty() {
            self.skin.update(queue, joint_matrices);
        }
        if !morph_weights.is_empty() {
            self.morph.update(queue, morph_weights);
        }
    }

    /// Render the 3D scene to a texture view (does not acquire or present the surface).
    pub fn render_to_view(&self, ctx: &RenderContext, view: &wgpu::TextureView) {
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, self.skin.bind_group(), &[]);
            pass.set_bind_group(2, self.morph.bind_group(), &[]);
            for (i, mesh) in self.meshes.iter().enumerate() {
                // Bind per-mesh material (texture + base color)
                if let Some(mat) = self.materials.get(i) {
                    pass.set_bind_group(3, &mat.bind_group, &[]);
                }
                mesh.draw(&mut pass);
            }
        }
        ctx.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Acquire surface, render, and present (convenience wrapper for non-overlay usage).
    pub fn render(&self, ctx: &RenderContext) -> anyhow::Result<()> {
        let output = ctx.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.render_to_view(ctx, &view);
        output.present();
        Ok(())
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.depth.resize(device, width, height);
    }
}
