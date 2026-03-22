use std::sync::Mutex;

use glam::Vec3;
use image::DynamicImage;
use ort::session::Session;
use ort::value::TensorRef;

use crate::preprocess;

/// Minimum confidence score to accept a hand detection.
const HAND_CONFIDENCE_THRESHOLD: f32 = 0.7;

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

        if outputs.len() < 2 {
            return Ok(None);
        }

        // output[0]: landmarks [1, 63] (21 * 3)
        // output[1]: hand confidence [1, 1] — sigmoid score for hand presence
        let (_, raw_data) = outputs[0].try_extract_tensor::<f32>()?;
        if raw_data.len() < 21 * 3 {
            return Ok(None);
        }

        // Check confidence score — reject low-confidence detections
        let (_, confidence_data) = outputs[1].try_extract_tensor::<f32>()?;
        let confidence = if !confidence_data.is_empty() {
            // Apply sigmoid if raw logit (MediaPipe outputs logit, not probability)
            let raw = confidence_data[0];
            1.0 / (1.0 + (-raw).exp())
        } else {
            0.0
        };
        log::trace!("Hand confidence: {:.4} (threshold: {:.2})", confidence, HAND_CONFIDENCE_THRESHOLD);
        if confidence < HAND_CONFIDENCE_THRESHOLD {
            return Ok(None);
        }

        let mut landmarks = preprocess::normalize_landmarks(raw_data, 21, 224.0, 224.0);

        // Sanity check: reject if landmarks are too clustered (likely noise/false positive).
        // A real hand's landmarks should be spatially spread out.
        let mean_x = landmarks.iter().map(|l| l.x).sum::<f32>() / 21.0;
        let mean_y = landmarks.iter().map(|l| l.y).sum::<f32>() / 21.0;
        let variance = landmarks.iter().map(|l| {
            (l.x - mean_x).powi(2) + (l.y - mean_y).powi(2)
        }).sum::<f32>() / 21.0;
        // Minimum spread: if all landmarks are within a tiny area, it's noise
        if variance < 0.005 {
            return Ok(None);
        }

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
