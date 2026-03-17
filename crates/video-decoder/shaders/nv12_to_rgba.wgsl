@group(0) @binding(0) var y_tex: texture_2d<f32>;
@group(0) @binding(1) var uv_tex: texture_2d<f32>;
@group(0) @binding(2) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(out_tex);
    if gid.x >= dims.x || gid.y >= dims.y {
        return;
    }
    let y = textureLoad(y_tex, vec2<i32>(gid.xy), 0).r;
    let uv = textureLoad(uv_tex, vec2<i32>(gid.xy / 2u), 0).rg;
    let u = uv.x - 0.5;
    let v = uv.y - 0.5;
    // BT.709 coefficients
    let r = y + 1.5748 * v;
    let g = y - 0.1873 * u - 0.4681 * v;
    let b = y + 1.8556 * u;
    textureStore(out_tex, vec2<i32>(gid.xy), vec4<f32>(clamp(r, 0.0, 1.0), clamp(g, 0.0, 1.0), clamp(b, 0.0, 1.0), 1.0));
}
