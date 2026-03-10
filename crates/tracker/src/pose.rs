use std::sync::Mutex;

use glam::{Vec2, Vec3};
use image::DynamicImage;
use ort::session::Session;
use ort::value::TensorRef;

use crate::preprocess;

/// Result of pose landmark detection: (3D world landmarks, 2D screen landmarks).
pub type PoseResult = (Option<Vec<Vec3>>, Option<Vec<Vec2>>);

/// Pose detector using ONNX Runtime.
///
/// Detects 33 pose landmarks (3D in meters + 2D normalized).
/// The session is wrapped in a `Mutex` to allow `&self` detect calls,
/// enabling parallel inference with other detectors via `rayon::join`.
pub struct PoseDetector {
    session: Mutex<Session>,
}

impl PoseDetector {
    /// Initialize from an ONNX model file.
    pub fn new(model_path: &str) -> anyhow::Result<Self> {
        let session = Session::builder()?.commit_from_file(model_path)?;
        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Detect pose landmarks from a camera frame.
    ///
    /// Returns (3D world landmarks, 2D screen landmarks), either of which may be None.
    pub fn detect(&self, frame: &DynamicImage) -> anyhow::Result<PoseResult> {
        let input_tensor = preprocess::preprocess_image(frame, 256, 256);
        let input_ref = TensorRef::from_array_view(&input_tensor)?;

        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs![input_ref])?;

        // Parse 3D world landmarks from output[0]: [1, 33*5] (x, y, z, visibility, presence)
        let landmarks_3d = if outputs.len() > 0 {
            let (_, raw_data) = outputs[0].try_extract_tensor::<f32>()?;
            if raw_data.len() >= 33 * 5 {
                let lm: Vec<Vec3> = (0..33)
                    .map(|i| {
                        let offset = i * 5;
                        Vec3::new(raw_data[offset], raw_data[offset + 1], raw_data[offset + 2])
                    })
                    .collect();
                Some(lm)
            } else if raw_data.len() >= 33 * 3 {
                let lm = preprocess::normalize_landmarks(raw_data, 33, 256.0, 256.0);
                Some(lm)
            } else {
                None
            }
        } else {
            None
        };

        // Parse 2D screen landmarks from output[1] if available
        let landmarks_2d = if outputs.len() > 1 {
            let (_, raw_data) = outputs[1].try_extract_tensor::<f32>()?;
            if raw_data.len() >= 33 * 2 {
                let stride = raw_data.len() / 33;
                let lm: Vec<Vec2> = (0..33)
                    .map(|i| {
                        let offset = i * stride;
                        Vec2::new(raw_data[offset], raw_data[offset + 1])
                    })
                    .collect();
                Some(lm)
            } else {
                None
            }
        } else {
            None
        };

        Ok((landmarks_3d, landmarks_2d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_invalid_path_returns_error() {
        let result = PoseDetector::new("/nonexistent/model.onnx");
        assert!(result.is_err());
    }
}
