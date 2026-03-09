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

// Material uniform (group 3, binding 0) - includes MToon toon shading parameters
struct MaterialUniform {
    base_color: vec4<f32>,
    shade_color: vec4<f32>,
    rim_color: vec4<f32>,
    // shade_shift, shade_toony, rim_power, rim_lift packed into a vec4
    mtoon_params: vec4<f32>,
};
@group(3) @binding(0) var<uniform> material: MaterialUniform;

// Texture and sampler (group 3, binding 1 and 2)
@group(3) @binding(1) var t_base_color: texture_2d<f32>;
@group(3) @binding(2) var s_base_color: sampler;

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
    // Sample base color texture and multiply by material base color
    let tex_color = textureSample(t_base_color, s_base_color, in.uv);
    let base = tex_color * material.base_color;

    // Unpack MToon parameters
    let shade_shift = material.mtoon_params.x;
    let shade_toony = material.mtoon_params.y;
    let rim_power = material.mtoon_params.z;
    let rim_lift = material.mtoon_params.w;

    // Directional light
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let n = normalize(in.world_normal);
    let ndotl = dot(n, light_dir);

    // MToon 2-step toon shading
    let shade_threshold = shade_shift + shade_toony;
    let shade_factor = smoothstep(shade_shift, shade_threshold, ndotl);
    let lit_color = mix(material.shade_color.rgb * base.rgb, base.rgb, shade_factor);

    // MToon rim light
    let view_dir = normalize(-in.world_pos);
    let ndotv = max(dot(n, view_dir), 0.0);
    let rim = pow(1.0 - ndotv, rim_power) * (1.0 + rim_lift);
    let rim_contribution = material.rim_color.rgb * rim;

    return vec4<f32>(lit_color + rim_contribution, base.a);
}
