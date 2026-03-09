use wgpu::util::DeviceExt;

pub struct MorphData {
    weight_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl MorphData {
    pub fn new(device: &wgpu::Device, max_targets: usize) -> Self {
        let buffer_size = (max_targets * std::mem::size_of::<f32>()) as u64;

        let weight_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("morph_weight_buffer"),
            contents: &vec![0u8; buffer_size as usize],
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("morph_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("morph_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: weight_buffer.as_entire_binding(),
            }],
        });

        Self {
            weight_buffer,
            bind_group,
            bind_group_layout,
        }
    }

    pub fn update(&self, queue: &wgpu::Queue, weights: &[f32]) {
        queue.write_buffer(&self.weight_buffer, 0, bytemuck::cast_slice(weights));
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}
