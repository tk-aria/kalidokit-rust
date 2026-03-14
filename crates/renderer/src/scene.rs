use crate::camera::CameraUniform;
use crate::context::RenderContext;
use crate::depth::DepthTexture;
use crate::light::StageLighting;
use crate::mesh::GpuMesh;
use crate::morph::{MorphBindGroupLayout, PerMeshMorph};
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
    per_mesh_morphs: Vec<PerMeshMorph>,
    depth: DepthTexture,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    lights_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    // Kept alive for potential future dynamic material creation.
    #[allow(dead_code)]
    material_bind_group_layout: wgpu::BindGroupLayout,
    /// Staging resources for reading back rendered frames (virtual camera).
    frame_capture_texture: Option<wgpu::Texture>,
    frame_capture_depth: Option<DepthTexture>,
    /// Double-buffered staging buffers for async GPU readback.
    /// Index alternates each frame to overlap copy and readback.
    frame_capture_buffers: [Option<wgpu::Buffer>; 2],
    frame_capture_buf_idx: usize,
    /// Signals that the previous buffer's map_async callback has fired.
    frame_capture_map_ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Whether the previous buffer has a pending readback.
    frame_capture_pending: bool,
    frame_capture_width: u32,
    frame_capture_height: u32,
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
    /// Create a new scene.
    ///
    /// `mesh_morph_targets[i]` contains the morph target position deltas for mesh `i`.
    /// Each inner `Vec<[f32; 3]>` has one `[dx, dy, dz]` per vertex for that morph target.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
        vertices_list: &[(&[Vertex], &[u32])],
        mesh_materials: &[MeshMaterialInput],
        mesh_morph_targets: &[Vec<Vec<[f32; 3]>>],
        max_joints: usize,
        stage_lighting: &StageLighting,
    ) -> Self {
        let meshes: Vec<GpuMesh> = vertices_list
            .iter()
            .map(|(verts, indices)| GpuMesh::from_vertices_indices(device, verts, indices))
            .collect();

        let skin = SkinData::new(device, max_joints.max(1));
        let morph_layout = MorphBindGroupLayout::new(device);
        let depth = DepthTexture::new(device, config.width, config.height);

        // Create per-mesh morph target data
        let per_mesh_morphs: Vec<PerMeshMorph> = vertices_list
            .iter()
            .enumerate()
            .map(|(i, (verts, _))| {
                let targets = mesh_morph_targets.get(i);
                let empty_targets = Vec::new();
                let target_deltas: &[Vec<[f32; 3]>] = match targets {
                    Some(t) => t.as_slice(),
                    None => &empty_targets,
                };
                PerMeshMorph::new(device, &morph_layout, verts.len(), target_deltas)
            })
            .collect();

        // Camera uniform
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_buffer"),
            contents: bytemuck::bytes_of(&CameraUniform::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Lights uniform (shares bind group 0 with camera)
        let lights_uniform = stage_lighting.to_uniform();
        let lights_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lights_buffer"),
            contents: bytemuck::bytes_of(&lights_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_lights_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_lights_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights_buffer.as_entire_binding(),
                },
            ],
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
                morph_layout.layout(),
                &material_bind_group_layout,
            ],
            Some(crate::depth::DEPTH_FORMAT),
        );

        Self {
            meshes,
            materials,
            skin,
            per_mesh_morphs,
            depth,
            pipeline,
            camera_buffer,
            lights_buffer,
            camera_bind_group,
            material_bind_group_layout,
            frame_capture_texture: None,
            frame_capture_depth: None,
            frame_capture_buffers: [None, None],
            frame_capture_buf_idx: 0,
            frame_capture_map_ready: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            frame_capture_pending: false,
            frame_capture_width: 0,
            frame_capture_height: 0,
        }
    }

    /// Update GPU buffers before rendering.
    ///
    /// `per_mesh_morph_weights[i]` contains the morph weights for mesh `i`.
    pub fn prepare(
        &self,
        queue: &wgpu::Queue,
        joint_matrices: &[Mat4],
        per_mesh_morph_weights: &[Vec<f32>],
        camera_uniform: &CameraUniform,
        stage_lighting: &StageLighting,
    ) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(camera_uniform));
        if !joint_matrices.is_empty() {
            self.skin.update(queue, joint_matrices);
        }
        for (i, morph) in self.per_mesh_morphs.iter().enumerate() {
            if let Some(weights) = per_mesh_morph_weights.get(i) {
                morph.update(queue, weights);
            }
        }
        let lights_uniform = stage_lighting.to_uniform();
        queue.write_buffer(&self.lights_buffer, 0, bytemuck::bytes_of(&lights_uniform));
    }

    /// Render the 3D scene to a texture view (does not acquire or present the surface).
    pub fn render_to_view(&self, ctx: &RenderContext, view: &wgpu::TextureView) {
        self.render_to_view_with_depth(ctx, view, &self.depth.view);
    }

    /// Render the 3D scene to a texture view with a specified depth buffer.
    fn render_to_view_with_depth(&self, ctx: &RenderContext, view: &wgpu::TextureView, depth_view: &wgpu::TextureView) {
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
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.12,
                            g: 0.12,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
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
            for (i, mesh) in self.meshes.iter().enumerate() {
                // Per-mesh morph target bind group
                if let Some(morph) = self.per_mesh_morphs.get(i) {
                    pass.set_bind_group(2, morph.bind_group(), &[]);
                }
                // Per-mesh material (texture + base color)
                if let Some(mat) = self.materials.get(i) {
                    pass.set_bind_group(3, &mat.bind_group, &[]);
                }
                mesh.draw(&mut pass);
            }
        }
        ctx.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render the 3D scene to the frame capture texture (uses its own depth buffer).
    pub fn render_to_capture(&self, ctx: &RenderContext) {
        if let (Some(view), Some(depth)) = (self.frame_capture_view(), &self.frame_capture_depth) {
            self.render_to_view_with_depth(ctx, &view, &depth.view);
        }
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

    /// Ensure the frame capture texture and double staging buffers exist at the given dimensions.
    pub fn ensure_frame_capture(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.frame_capture_width == width && self.frame_capture_height == height && self.frame_capture_texture.is_some() {
            return;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame_capture_texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Bytes per row must be aligned to 256 (wgpu requirement)
        let bytes_per_row = (width * 4 + 255) & !255;
        let buf_size = (bytes_per_row * height) as u64;
        let make_buffer = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: buf_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        };

        self.frame_capture_texture = Some(texture);
        self.frame_capture_buffers = [Some(make_buffer("frame_capture_buf_0")), Some(make_buffer("frame_capture_buf_1"))];
        self.frame_capture_buf_idx = 0;
        self.frame_capture_map_ready.store(false, std::sync::atomic::Ordering::Release);
        self.frame_capture_pending = false;
        self.frame_capture_depth = Some(DepthTexture::new(device, width, height));
        self.frame_capture_width = width;
        self.frame_capture_height = height;
    }

    /// Get the frame capture texture view for rendering.
    pub fn frame_capture_view(&self) -> Option<wgpu::TextureView> {
        self.frame_capture_texture.as_ref().map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
    }

    /// Copy the rendered frame capture texture into the current staging buffer
    /// and initiate an async map request. Returns the previous frame's BGRA data if available.
    ///
    /// Double-buffer flow: copy into buffer[idx], read back buffer[1-idx] from previous frame.
    /// Uses non-blocking poll — if the previous frame's mapping isn't ready, skips the readback.
    pub fn capture_frame_async(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Vec<u8>> {
        let texture = self.frame_capture_texture.as_ref()?;
        let width = self.frame_capture_width;
        let height = self.frame_capture_height;
        let aligned_bpr = (width * 4 + 255) & !255;
        let cur = self.frame_capture_buf_idx;
        let prev = 1 - cur;

        // 1. Non-blocking poll to process GPU callbacks (map_async completion)
        device.poll(wgpu::Maintain::Poll);

        // 2. Try to read back the previous buffer if mapping is complete
        let result = if self.frame_capture_pending
            && self.frame_capture_map_ready.load(std::sync::atomic::Ordering::Acquire)
        {
            if let Some(buf) = &self.frame_capture_buffers[prev] {
                let data = buf.slice(..).get_mapped_range();
                let mut pixels = Vec::with_capacity((width * height * 4) as usize);
                for row in 0..height {
                    let start = (row * aligned_bpr) as usize;
                    let end = start + (width * 4) as usize;
                    pixels.extend_from_slice(&data[start..end]);
                }
                drop(data);
                buf.unmap();
                self.frame_capture_map_ready.store(false, std::sync::atomic::Ordering::Release);
                Some(pixels)
            } else {
                None
            }
        } else {
            None
        };

        // 3. Copy texture → current staging buffer
        if let Some(buf) = &self.frame_capture_buffers[cur] {
            let mut encoder = device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("vcam_copy") },
            );
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(aligned_bpr),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            );
            queue.submit(std::iter::once(encoder.finish()));

            // 4. Start async map on the current buffer with completion signal
            let ready = self.frame_capture_map_ready.clone();
            buf.slice(..).map_async(wgpu::MapMode::Read, move |_| {
                ready.store(true, std::sync::atomic::Ordering::Release);
            });
        }

        // 5. Swap buffers
        self.frame_capture_buf_idx = prev;
        self.frame_capture_pending = true;

        result
    }
}
