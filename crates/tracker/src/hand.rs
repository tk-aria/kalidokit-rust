use glam::Vec3;
use image::DynamicImage;
use ort::session::Session;

/// Detect 21 hand landmarks for a single hand.
///
/// Note: `is_left` refers to the actual hand side. Due to camera mirroring,
/// MediaPipe's rightHandLandmarks correspond to the user's left hand.
pub fn detect(
    _session: &Session,
    _frame: &DynamicImage,
    _is_left: bool,
) -> anyhow::Result<Option<Vec<Vec3>>> {
    // 1. Preprocess frame to model input size
    // 2. Run ONNX inference
    // 3. Parse output tensor to Vec<Vec3> landmarks
    todo!("Implement hand landmark ONNX inference")
}
