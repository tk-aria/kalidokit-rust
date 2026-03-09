use crate::{face_mesh, hand, pose, HolisticResult};
use image::DynamicImage;
use ort::session::Session;

/// Combined inference pipeline for face, pose, and hand tracking.
pub struct HolisticTracker {
    face_session: Session,
    pose_session: Session,
    hand_session: Session,
}

impl HolisticTracker {
    /// Initialize tracker from ONNX model files.
    pub fn new(
        face_model_path: &str,
        pose_model_path: &str,
        hand_model_path: &str,
    ) -> anyhow::Result<Self> {
        let face_session = Session::builder()?
            .with_model_from_file(face_model_path)?;
        let pose_session = Session::builder()?
            .with_model_from_file(pose_model_path)?;
        let hand_session = Session::builder()?
            .with_model_from_file(hand_model_path)?;

        Ok(Self {
            face_session,
            pose_session,
            hand_session,
        })
    }

    /// Detect all landmarks from a camera frame.
    pub fn detect(&self, frame: &DynamicImage) -> anyhow::Result<HolisticResult> {
        let face_landmarks = face_mesh::detect(&self.face_session, frame)?;
        let (pose_3d, pose_2d) = pose::detect(&self.pose_session, frame)?;
        // Note: hand landmarks are swapped due to camera mirror
        let left_hand = hand::detect(&self.hand_session, frame, true)?;
        let right_hand = hand::detect(&self.hand_session, frame, false)?;

        Ok(HolisticResult {
            face_landmarks,
            pose_landmarks_3d: pose_3d,
            pose_landmarks_2d: pose_2d,
            left_hand_landmarks: left_hand,
            right_hand_landmarks: right_hand,
        })
    }
}
