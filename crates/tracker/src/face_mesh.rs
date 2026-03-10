use std::sync::Mutex;

use glam::Vec3;
use image::DynamicImage;
use ort::session::Session;
use ort::value::TensorRef;

use crate::preprocess;

/// Face mesh detector using ONNX Runtime.
///
/// Detects 468 (or 478 with iris) face landmarks from an image.
/// The session is wrapped in a `Mutex` to allow `&self` detect calls,
/// enabling parallel inference with other detectors via `rayon::join`.
pub struct FaceMeshDetector {
    session: Mutex<Session>,
}

impl FaceMeshDetector {
    /// Initialize from an ONNX model file.
    pub fn new(model_path: &str) -> anyhow::Result<Self> {
        let session = Session::builder()?.commit_from_file(model_path)?;
        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Detect face landmarks from a camera frame.
    ///
    /// Returns `None` if no face is detected (insufficient landmarks).
    pub fn detect(&self, frame: &DynamicImage) -> anyhow::Result<Option<Vec<Vec3>>> {
        let input_tensor = preprocess::preprocess_image(frame, 192, 192);
        let input_ref = TensorRef::from_array_view(&input_tensor)?;

        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs![input_ref])?;

        // MediaPipe face mesh outputs:
        // output[0]: landmarks [1, 1404] (468 * 3) or [1, 1434] (478 * 3)
        let (_, raw_data) = outputs[0].try_extract_tensor::<f32>()?;

        if raw_data.is_empty() {
            return Ok(None);
        }

        // Determine number of landmarks (468 or 478)
        let num_landmarks = raw_data.len() / 3;
        if num_landmarks < 468 {
            return Ok(None);
        }

        let landmarks = preprocess::normalize_landmarks(raw_data, num_landmarks, 192.0, 192.0);
        Ok(Some(landmarks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_invalid_path_returns_error() {
        let result = FaceMeshDetector::new("/nonexistent/model.onnx");
        assert!(result.is_err());
    }
}
