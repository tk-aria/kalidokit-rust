// Camera uniform (group 0, binding 0)
struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    eye_pos: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

// Joint matrices (group 1, binding 0) - max 256 bones
@group(1) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;

// Morph weights (group 2, binding 0)
@group(2) @binding(0) var<storage, read> morph_weights: array<f32>;

// Morph target position deltas (group 2, binding 1)
// Layout: deltas[target_index * num_vertices + vertex_index].xyz
@group(2) @binding(1) var<storage, read> morph_deltas: array<vec4<f32>>;

// Morph info (group 2, binding 2)
struct MorphInfo {
    num_vertices: u32,
    num_targets: u32,
};
@group(2) @binding(2) var<uniform> morph_info: MorphInfo;

// Stage lights (group 0, binding 1) - 3 configurable lights
struct LightsUniform {
    // light[i]: dir_intensity = xyz direction + w intensity, color = rgb + w pad
    light0_dir_intensity: vec4<f32>,
    light0_color: vec4<f32>,
    light1_dir_intensity: vec4<f32>,
    light1_color: vec4<f32>,
    light2_dir_intensity: vec4<f32>,
    light2_color: vec4<f32>,
};
@group(0) @binding(1) var<uniform> lights: LightsUniform;

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
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) joint_indices: vec4<u32>,
    @location(4) joint_weights: vec4<f32>,
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

    // Apply morph target displacements to position
    var pos = in.position;
    for (var t = 0u; t < morph_info.num_targets; t = t + 1u) {
        let w = morph_weights[t];
        if (w > 0.001) {
            let delta = morph_deltas[t * morph_info.num_vertices + in.vertex_index];
            pos = pos + delta.xyz * w;
        }
    }

    let weight_sum = in.joint_weights.x + in.joint_weights.y + in.joint_weights.z + in.joint_weights.w;

    var world_pos: vec4<f32>;
    var world_normal: vec3<f32>;

    if (weight_sum > 0.0) {
        // Linear Blend Skinning
        let skin_matrix =
            joint_matrices[in.joint_indices.x] * in.joint_weights.x +
            joint_matrices[in.joint_indices.y] * in.joint_weights.y +
            joint_matrices[in.joint_indices.z] * in.joint_weights.z +
            joint_matrices[in.joint_indices.w] * in.joint_weights.w;

        world_pos = skin_matrix * vec4<f32>(pos, 1.0);
        let normal_mat = mat3x3<f32>(
            skin_matrix[0].xyz,
            skin_matrix[1].xyz,
            skin_matrix[2].xyz,
        );
        world_normal = normalize(normal_mat * in.normal);
    } else {
        // No skinning - use model matrix
        world_pos = camera.model * vec4<f32>(pos, 1.0);
        let normal_matrix = mat3x3<f32>(
            camera.model[0].xyz,
            camera.model[1].xyz,
            camera.model[2].xyz,
        );
        world_normal = normalize(normal_matrix * in.normal);
    }

    out.clip_position = camera.view_proj * world_pos;
    out.world_pos = world_pos.xyz;
    out.world_normal = world_normal;
    out.uv = in.uv;

    return out;
}

// ── Helpers ──────────────────────────────────────────────────

const PI: f32 = 3.14159265;

// Cel-shade: hard 2-step with very narrow transition
fn cel_shade(ndotl: f32, shift: f32, width: f32) -> f32 {
    return smoothstep(shift, shift + width, ndotl);
}

// Saturate color: boost saturation for vivid stage look
fn saturate_color(c: vec3<f32>, amount: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return mix(vec3<f32>(luma), c, 1.0 + amount);
}

// ── Classic MToon shading ──
// Single directional light, 2-step toon, simple rim.
fn shade_classic(
    albedo: vec3<f32>,
    base_a: f32,
    n: vec3<f32>,
    world_pos: vec3<f32>,
    shade_shift: f32,
    shade_toony: f32,
    rim_power: f32,
    rim_lift: f32,
) -> vec4<f32> {
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let ndotl = dot(n, light_dir);

    let shade_threshold = shade_shift + shade_toony;
    let shade_factor = smoothstep(shade_shift, shade_threshold, ndotl);
    let lit_color = mix(material.shade_color.rgb * albedo, albedo, shade_factor);

    let view_dir = normalize(-world_pos);
    let ndotv = max(dot(n, view_dir), 0.0);
    let rim = pow(1.0 - ndotv, rim_power) * (1.0 + rim_lift);
    let rim_contribution = material.rim_color.rgb * rim;

    return vec4<f32>(lit_color + rim_contribution, base_a);
}

// ── Virtual Live toon shading ──
// 3-point stage lighting, cel-shade, anime specular, colored rim.
fn shade_virtual_live(
    albedo: vec3<f32>,
    base_a: f32,
    n: vec3<f32>,
    v: vec3<f32>,
    n_dot_v: f32,
    shade_shift: f32,
    rim_power: f32,
    rim_lift: f32,
) -> vec4<f32> {
    // Key light (light 0)
    let key_dir = normalize(lights.light0_dir_intensity.xyz);
    let key_color = lights.light0_color.rgb;
    let key_intensity = lights.light0_dir_intensity.w;

    // Fill light (light 1)
    let fill_dir = normalize(lights.light1_dir_intensity.xyz);
    let fill_color = lights.light1_color.rgb;
    let fill_intensity = lights.light1_dir_intensity.w;

    // Back light (light 2)
    let back_dir = normalize(lights.light2_dir_intensity.xyz);
    let back_color = lights.light2_color.rgb;
    let back_intensity = lights.light2_dir_intensity.w;

    let n_dot_l_key = max(dot(n, key_dir), 0.0);
    let n_dot_l_fill = max(dot(n, fill_dir), 0.0);
    let n_dot_l_back = max(dot(n, back_dir), 0.0);

    // Cel-shaded diffuse per light
    let cel_key = cel_shade(n_dot_l_key, shade_shift, 0.05);
    let cel_fill = cel_shade(n_dot_l_fill, -0.1, 0.15);
    let cel_back = cel_shade(n_dot_l_back, -0.2, 0.2);

    let shade_tint = material.shade_color.rgb * albedo * 0.6;

    let diffuse_key = mix(shade_tint, albedo, cel_key) * key_color * key_intensity;
    let diffuse_fill = mix(shade_tint * 0.4, albedo * 0.5, cel_fill) * fill_color * fill_intensity;
    let diffuse_back = albedo * cel_back * back_color * back_intensity * 0.3;

    // Anime-style sharp specular
    let h_key = normalize(v + key_dir);
    let n_dot_h = max(dot(n, h_key), 0.0);
    let spec_raw = pow(n_dot_h, 80.0);
    let spec_toon = smoothstep(0.3, 0.5, spec_raw);
    let specular = mix(key_color, vec3<f32>(1.0, 1.0, 1.0), 0.7) * spec_toon * 0.8;

    // Ambient: dark stage floor bounce
    let ambient_up = vec3<f32>(0.15, 0.14, 0.20);
    let ambient_down = vec3<f32>(0.05, 0.04, 0.06);
    let ambient = mix(ambient_down, ambient_up, n.y * 0.5 + 0.5) * albedo;

    // Rim light: vivid colored edge
    let fresnel = pow(1.0 - n_dot_v, rim_power + 1.0) * (1.0 + rim_lift);
    let rim_cel = smoothstep(0.15, 0.4, fresnel);
    let rim_mix_color = mix(material.rim_color.rgb, back_color, 0.4);
    let rim = rim_mix_color * rim_cel * 0.9;

    // Top highlight: hair/head sheen
    let top_dot = max(dot(n, vec3<f32>(0.0, 1.0, 0.0)), 0.0);
    let top_sheen = smoothstep(0.75, 0.88, top_dot);
    let top_highlight = key_color * top_sheen * 0.15;

    // Composition
    var color = ambient + diffuse_key + diffuse_fill + diffuse_back
        + specular + rim + top_highlight;

    // Boost saturation
    color = saturate_color(color, 0.15);

    // Soft shoulder tone mapping
    let peak = vec3<f32>(2.0);
    color = color * (vec3<f32>(1.0) + color / (peak * peak)) / (vec3<f32>(1.0) + color);

    return vec4<f32>(color, base_a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_base_color, s_base_color, in.uv);
    let base = tex_color * material.base_color;
    let albedo = base.rgb;

    let shade_shift = material.mtoon_params.x;
    let shade_toony = material.mtoon_params.y;
    let rim_power = material.mtoon_params.z;
    let rim_lift = material.mtoon_params.w;

    let n = normalize(in.world_normal);

    // Shading mode: 0.0 = VirtualLive, 1.0 = Classic
    let shading_mode = lights.light2_color.w;

    if (shading_mode > 0.5) {
        return shade_classic(albedo, base.a, n, in.world_pos, shade_shift, shade_toony, rim_power, rim_lift);
    } else {
        let v = normalize(camera.eye_pos.xyz - in.world_pos);
        let n_dot_v = max(dot(n, v), 0.0);
        return shade_virtual_live(albedo, base.a, n, v, n_dot_v, shade_shift, rim_power, rim_lift);
    }
}
