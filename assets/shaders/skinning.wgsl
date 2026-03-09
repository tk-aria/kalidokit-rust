// Camera uniform (group 0, binding 0)
struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

// Joint matrices (group 1, binding 0) - max 256 bones
@group(1) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;

// Morph weights (group 2, binding 0) - max 64 targets
@group(2) @binding(0) var<storage, read> morph_weights: array<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = camera.model * vec4<f32>(in.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;

    let normal_matrix = mat3x3<f32>(
        camera.model[0].xyz,
        camera.model[1].xyz,
        camera.model[2].xyz,
    );
    out.world_normal = normalize(normal_matrix * in.normal);
    out.uv = in.uv;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let ndl = max(dot(in.world_normal, light_dir), 0.0);
    let ambient = 0.15;
    let diffuse = ndl * 0.85;
    let color = vec3<f32>(1.0, 1.0, 1.0) * (ambient + diffuse);
    return vec4<f32>(color, 1.0);
}
