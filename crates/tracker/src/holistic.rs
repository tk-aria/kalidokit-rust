use crate::{face_mesh::FaceMeshDetector, hand::HandDetector, pose::PoseDetector, HolisticResult};
use image::DynamicImage;

/// Combined inference pipeline for face, pose, and hand tracking.
pub struct HolisticTracker {
    face_detector: FaceMeshDetector,
    pose_detector: PoseDetector,
    left_hand_detector: HandDetector,
    right_hand_detector: HandDetector,
}

impl HolisticTracker {
    /// Initialize tracker from ONNX model files.
    pub fn new(
        face_model_path: &str,
        pose_model_path: &str,
        hand_model_path: &str,
    ) -> anyhow::Result<Self> {
        let face_detector = FaceMeshDetector::new(face_model_path)?;
        let pose_detector = PoseDetector::new(pose_model_path)?;
        let left_hand_detector = HandDetector::new(hand_model_path)?;
        let right_hand_detector = HandDetector::new(hand_model_path)?;

        Ok(Self {
            face_detector,
            pose_detector,
            left_hand_detector,
            right_hand_detector,
        })
    }

    /// Detect all landmarks from a camera frame.
    ///
    /// Each detector runs independently; if one fails, others still produce results.
    pub fn detect(&mut self, frame: &DynamicImage) -> anyhow::Result<HolisticResult> {
        let face_landmarks = self.face_detector.detect(frame).unwrap_or(None);
        let (pose_3d, pose_2d) = self.pose_detector.detect(frame).unwrap_or((None, None));
        let left_hand = self.left_hand_detector.detect(frame, true).unwrap_or(None);
        let right_hand = self
            .right_hand_detector
            .detect(frame, false)
            .unwrap_or(None);

        Ok(HolisticResult {
            face_landmarks,
            pose_landmarks_3d: pose_3d,
            pose_landmarks_2d: pose_2d,
            left_hand_landmarks: left_hand,
            right_hand_landmarks: right_hand,
        })
    }
}
