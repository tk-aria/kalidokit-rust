/// Clamp a value between min and max.
pub fn clamp(val: f32, min: f32, max: f32) -> f32 {
    val.max(min).min(max)
}

/// Remap a value from one range to another.
pub fn remap(val: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = (val - in_min) / (in_max - in_min);
    out_min + t * (out_max - out_min)
}

/// Linear interpolation between two f32 values.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation between two Vec3 values.
pub fn lerp_vec3(a: glam::Vec3, b: glam::Vec3, t: f32) -> glam::Vec3 {
    a + (b - a) * t
}
