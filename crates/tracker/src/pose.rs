use glam::{Vec2, Vec3};
use image::DynamicImage;
use ort::session::Session;

/// Detect 33 pose landmarks (3D in meters + 2D normalized).
pub fn detect(
    _session: &Session,
    _frame: &DynamicImage,
) -> anyhow::Result<(Option<Vec<Vec3>>, Option<Vec<Vec2>>)> {
    // 1. Preprocess frame to model input size
    // 2. Run ONNX inference
    // 3. Parse output tensors to 3D world landmarks and 2D screen landmarks
    todo!("Implement pose landmark ONNX inference")
}
