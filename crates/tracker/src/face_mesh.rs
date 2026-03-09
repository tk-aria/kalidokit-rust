use glam::Vec3;
use image::DynamicImage;
use ort::session::Session;

/// Detect 468 (or 478 with iris) face landmarks from an image.
pub fn detect(
    _session: &Session,
    _frame: &DynamicImage,
) -> anyhow::Result<Option<Vec<Vec3>>> {
    // 1. Preprocess frame to model input size
    // 2. Run ONNX inference
    // 3. Parse output tensor to Vec<Vec3> landmarks
    todo!("Implement face mesh ONNX inference")
}
