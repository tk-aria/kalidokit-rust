use crate::camera::CameraUniform;
use crate::context::RenderContext;
use crate::depth::DepthTexture;
use crate::mesh::GpuMesh;
use crate::morph::MorphData;
use crate::pipeline::create_render_pipeline;
use crate::skin::SkinData;
use crate::vertex::Vertex;
use glam::Mat4;
use wgpu::util::DeviceExt;

pub struct Scene {
    meshes: Vec<GpuMesh>,
    skin: SkinData,
    morph: MorphData,
    depth: DepthTexture,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
}

impl Scene {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        vertices_list: &[(&[Vertex], &[u32])],
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

        let shader_src = include_str!("../../../assets/shaders/skinning.wgsl");
        let pipeline = create_render_pipeline(
            device,
            config.format,
            shader_src,
            &[
                &camera_bind_group_layout,
                skin.bind_group_layout(),
                morph.bind_group_layout(),
            ],
            Some(crate::depth::DEPTH_FORMAT),
        );

        Self {
            meshes,
            skin,
            morph,
            depth,
            pipeline,
            camera_buffer,
            camera_bind_group,
        }
    }

    pub fn prepare(
        &self,
        queue: &wgpu::Queue,
        joint_matrices: &[Mat4],
        morph_weights: &[f32],
        camera_uniform: &CameraUniform,
    ) {
        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(camera_uniform),
        );
        if !joint_matrices.is_empty() {
            self.skin.update(queue, joint_matrices);
        }
        if !morph_weights.is_empty() {
            self.morph.update(queue, morph_weights);
        }
    }

    pub fn render(&self, ctx: &RenderContext) -> anyhow::Result<()> {
        let output = ctx.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            for mesh in &self.meshes {
                mesh.draw(&mut pass);
            }
        }
        ctx.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.depth.resize(device, width, height);
    }
}
