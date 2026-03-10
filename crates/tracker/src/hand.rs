use std::sync::Mutex;

use glam::Vec3;
use image::DynamicImage;
use ort::session::Session;
use ort::value::TensorRef;

use crate::preprocess;

/// Hand landmark detector using ONNX Runtime.
///
/// Detects 21 hand landmarks for a single hand.
/// The session is wrapped in a `Mutex` to allow `&self` detect calls,
/// enabling parallel inference with other detectors via `rayon::join`.
pub struct HandDetector {
    session: Mutex<Session>,
}

impl HandDetector {
    /// Initialize from an ONNX model file.
    pub fn new(model_path: &str) -> anyhow::Result<Self> {
        let session = Session::builder()?.commit_from_file(model_path)?;
        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Detect hand landmarks from a camera frame.
    ///
    /// `is_left` refers to the actual hand side. Due to camera mirroring,
    /// MediaPipe's rightHandLandmarks correspond to the user's left hand.
    ///
    /// Returns `None` if no hand is detected.
    pub fn detect(&self, frame: &DynamicImage, is_left: bool) -> anyhow::Result<Option<Vec<Vec3>>> {
        let input_tensor = preprocess::preprocess_image(frame, 224, 224);
        let input_ref = TensorRef::from_array_view(&input_tensor)?;

        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs![input_ref])?;

        if outputs.len() == 0 {
            return Ok(None);
        }

        // output[0]: landmarks [1, 63] (21 * 3)
        let (_, raw_data) = outputs[0].try_extract_tensor::<f32>()?;
        if raw_data.len() < 21 * 3 {
            return Ok(None);
        }

        let mut landmarks = preprocess::normalize_landmarks(raw_data, 21, 224.0, 224.0);

        // Mirror x coordinates for left hand (camera mirror compensation)
        if is_left {
            for lm in &mut landmarks {
                lm.x = 1.0 - lm.x;
            }
        }

        Ok(Some(landmarks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_invalid_path_returns_error() {
        let result = HandDetector::new("/nonexistent/model.onnx");
        assert!(result.is_err());
    }
}
