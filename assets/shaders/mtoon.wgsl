// MToon Toon Shader for VRM
// Based on VRM MToon material specification

// Camera uniform (group 0, binding 0)
struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

// Joint matrices (group 1, binding 0)
@group(1) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;

// Morph weights (group 2, binding 0)
@group(2) @binding(0) var<storage, read> morph_weights: array<f32>;

// MToon material parameters
struct MToonMaterial {
    color: vec4<f32>,           // Lit (base) color
    shade_color: vec4<f32>,     // Shade color
    shade_shift: f32,           // Shadow boundary shift [-1, 1]
    shade_toony: f32,           // Toon shading hardness [0, 1]
    rim_color_factor: f32,      // Rim light intensity
    rim_power: f32,             // Rim light falloff exponent
    rim_lift: f32,              // Rim light lift
    outline_width: f32,         // Outline width
    _pad0: f32,
    _pad1: f32,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) world_pos: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = camera.model * vec4<f32>(in.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;
    out.world_pos = world_pos.xyz;

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
    // MToon material defaults (hardcoded for now)
    let base_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    let shade_color = vec3<f32>(0.6, 0.6, 0.6);
    let shade_shift = -0.1;
    let shade_toony = 0.9;
    let rim_power = 5.0;
    let rim_lift = 0.0;

    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let ndotl = dot(normalize(in.world_normal), light_dir);

    // 2-step toon shading
    let shade_threshold = shade_shift + shade_toony;
    let shade_factor = smoothstep(shade_shift, shade_threshold, ndotl);
    let lit_color = mix(shade_color, base_color.rgb, shade_factor);

    // Rim light
    let view_dir = normalize(-in.world_pos);
    let rim = pow(1.0 - max(dot(normalize(in.world_normal), view_dir), 0.0), rim_power);
    let rim_color = vec3<f32>(1.0, 1.0, 1.0) * rim * (1.0 + rim_lift);

    return vec4<f32>(lit_color + rim_color * 0.1, base_color.a);
}

// Outline vertex shader (separate pass)
@vertex
fn vs_outline(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let outline_width = 0.002;
    let expanded_pos = in.position + in.normal * outline_width;
    let world_pos = camera.model * vec4<f32>(expanded_pos, 1.0);
    out.clip_position = camera.view_proj * world_pos;
    out.world_pos = world_pos.xyz;
    out.world_normal = in.normal;
    out.uv = in.uv;

    return out;
}

@fragment
fn fs_outline(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
