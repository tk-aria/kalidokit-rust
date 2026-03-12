use wgpu::util::DeviceExt;

/// Morph info uniform matching the shader's MorphInfo struct.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MorphInfo {
    num_vertices: u32,
    num_targets: u32,
    _pad: [u32; 2],
}

/// Shared bind group layout for per-mesh morph target data.
pub struct MorphBindGroupLayout {
    layout: wgpu::BindGroupLayout,
}

impl MorphBindGroupLayout {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("morph_bind_group_layout"),
            entries: &[
                // binding 0: morph_weights array<f32>
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: morph_deltas array<vec4<f32>>
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: morph_info uniform { num_vertices, num_targets }
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        Self { layout }
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}

/// Per-mesh morph target data on the GPU.
///
/// Stores morph weights (updated each frame), morph target position deltas
/// (immutable after creation), and morph info metadata.
pub struct PerMeshMorph {
    weight_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    num_targets: usize,
}

impl PerMeshMorph {
    /// Create per-mesh morph data from position deltas.
    ///
    /// `targets[t]` contains `[dx, dy, dz]` position deltas for morph target `t`,
    /// one entry per vertex in the same order as the vertex buffer.
    pub fn new(
        device: &wgpu::Device,
        layout: &MorphBindGroupLayout,
        num_vertices: usize,
        targets: &[Vec<[f32; 3]>],
    ) -> Self {
        let num_targets = targets.len().max(1);

        // Weight buffer (updated each frame via queue.write_buffer)
        let weight_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("morph_weight_buffer"),
            contents: &vec![0u8; num_targets * std::mem::size_of::<f32>()],
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Delta buffer: array<vec4<f32>>, layout: deltas[t * num_vertices + v]
        let mut delta_data: Vec<[f32; 4]> = Vec::with_capacity(num_targets * num_vertices);
        for target in targets {
            for v in 0..num_vertices {
                if v < target.len() {
                    let d = target[v];
                    delta_data.push([d[0], d[1], d[2], 0.0]);
                } else {
                    delta_data.push([0.0, 0.0, 0.0, 0.0]);
                }
            }
        }
        // If no targets, need at least one entry for a valid buffer
        if delta_data.is_empty() {
            delta_data.push([0.0, 0.0, 0.0, 0.0]);
        }

        let delta_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("morph_delta_buffer"),
            contents: bytemuck::cast_slice(&delta_data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Info uniform
        let info = MorphInfo {
            num_vertices: num_vertices as u32,
            num_targets: targets.len() as u32,
            _pad: [0; 2],
        };
        let info_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("morph_info_buffer"),
            contents: bytemuck::bytes_of(&info),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("morph_bind_group"),
            layout: layout.layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: weight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: delta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: info_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            weight_buffer,
            bind_group,
            num_targets: targets.len(),
        }
    }

    /// Update morph weights for this mesh.
    pub fn update(&self, queue: &wgpu::Queue, weights: &[f32]) {
        if !weights.is_empty() {
            queue.write_buffer(&self.weight_buffer, 0, bytemuck::cast_slice(weights));
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn num_targets(&self) -> usize {
        self.num_targets
    }
}
