use std::sync::Mutex;

use glam::{Vec2, Vec3};
use image::DynamicImage;
use log;
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
        pipeline_logger::tracker(log::Level::Trace, "pose input tensor")
            .field("shape", format!("{:?}", input_tensor.shape()))
            .emit();

        let input_ref = TensorRef::from_array_view(&input_tensor)?;

        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs![input_ref])?;

        pipeline_logger::tracker(log::Level::Debug, "pose outputs")
            .field("num_outputs", outputs.len())
            .emit();

        // Parse 3D world landmarks from output[0]: [1, 33*5] (x, y, z, visibility, presence)
        let landmarks_3d = if outputs.len() > 0 {
            let (shape, raw_data) = outputs[0].try_extract_tensor::<f32>()?;
            pipeline_logger::tracker(log::Level::Debug, "pose output[0]")
                .field("shape", format!("{:?}", shape))
                .field("data_len", raw_data.len())
                .field(
                    "sample",
                    if raw_data.len() >= 5 {
                        format!(
                            "[{:.3},{:.3},{:.3},{:.3},{:.3}]",
                            raw_data[0], raw_data[1], raw_data[2], raw_data[3], raw_data[4]
                        )
                    } else {
                        format!("{:?}", &raw_data[..raw_data.len().min(5)])
                    },
                )
                .emit();
            if raw_data.len() >= 33 * 5 {
                // Convert pixel-space landmarks to world-like coordinates.
                // MediaPipe Holistic JS world landmarks are centered at hip midpoint,
                // with X right (person's perspective), Y up, Z toward camera.
                // Our ONNX model outputs pixel coordinates (0-256, y-down).
                let hip_l_x = raw_data[23 * 5];
                let hip_l_y = raw_data[23 * 5 + 1];
                let hip_l_z = raw_data[23 * 5 + 2];
                let hip_r_x = raw_data[24 * 5];
                let hip_r_y = raw_data[24 * 5 + 1];
                let hip_r_z = raw_data[24 * 5 + 2];
                let cx = (hip_l_x + hip_r_x) * 0.5;
                let cy = (hip_l_y + hip_r_y) * 0.5;
                let cz = (hip_l_z + hip_r_z) * 0.5;

                let lm: Vec<Vec3> = (0..33)
                    .map(|i| {
                        let offset = i * 5;
                        // Negate x (pixel right→left = person's right)
                        // Negate y (pixel down→up)
                        // Keep z sign (positive = toward camera in both systems)
                        Vec3::new(
                            -(raw_data[offset] - cx) / 256.0,
                            -(raw_data[offset + 1] - cy) / 256.0,
                            (raw_data[offset + 2] - cz) / 256.0,
                        )
                    })
                    .collect();

                // Also produce 2D screen landmarks from raw pixel coordinates
                // (normalized to [0,1], NOT flipped — screen space convention).
                let lm2d: Vec<Vec2> = (0..33)
                    .map(|i| {
                        let offset = i * 5;
                        Vec2::new(raw_data[offset] / 256.0, raw_data[offset + 1] / 256.0)
                    })
                    .collect();

                return Ok((Some(lm), Some(lm2d)));
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
            let (shape2, raw_data) = outputs[1].try_extract_tensor::<f32>()?;
            pipeline_logger::tracker(log::Level::Debug, "pose output[1]")
                .field("shape", format!("{:?}", shape2))
                .field("data_len", raw_data.len())
                .field(
                    "sample",
                    if raw_data.len() >= 5 {
                        format!(
                            "[{:.3},{:.3},{:.3},{:.3},{:.3}]",
                            raw_data[0], raw_data[1], raw_data[2], raw_data[3], raw_data[4]
                        )
                    } else {
                        format!("{:?}", &raw_data[..raw_data.len().min(5)])
                    },
                )
                .emit();
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
