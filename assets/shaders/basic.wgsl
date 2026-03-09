struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = camera.view_proj * camera.model * vec4<f32>(pos, 1.0);
    out.normal = (camera.model * vec4<f32>(normal, 0.0)).xyz;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let ndotl = max(dot(normalize(in.normal), light_dir), 0.0);
    let color = vec3<f32>(0.8, 0.8, 0.8) * (0.3 + 0.7 * ndotl);
    return vec4<f32>(color, 1.0);
}
