use crate::{
    face_mesh::FaceMeshDetector,
    hand::HandDetector,
    pose::PoseDetector,
    preprocess::{calc_hand_roi, crop_image},
    HolisticResult,
};
use image::DynamicImage;
use log;

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
    /// When `face_only` is true, only face detection runs (pose/hand skipped).
    /// Otherwise, face and pose run in parallel, then hand detection uses pose ROI.
    pub fn detect(&self, frame: &DynamicImage) -> anyhow::Result<HolisticResult> {
        self.detect_with_mode(frame, false)
    }

    /// Detect with explicit mode control.
    pub fn detect_with_mode(
        &self,
        frame: &DynamicImage,
        face_only: bool,
    ) -> anyhow::Result<HolisticResult> {
        pipeline_logger::tracker(log::Level::Debug, "detection started")
            .field("frame_w", frame.width())
            .field("frame_h", frame.height())
            .field("mode", if face_only { "face_only" } else { "full" })
            .emit();

        let detect_start = std::time::Instant::now();

        // Face detection (always runs)
        let face_landmarks = match self.face_detector.detect(frame) {
            Ok(result) => result,
            Err(e) => {
                pipeline_logger::tracker(log::Level::Warn, "face detection error")
                    .field("error", format!("{e:#}"))
                    .emit();
                None
            }
        };

        // Skip pose/hand when face_only mode
        if face_only {
            let elapsed = detect_start.elapsed();
            pipeline_logger::tracker(log::Level::Debug, "detection complete (face only)")
                .field(
                    "elapsed_ms",
                    format!("{:.1}", elapsed.as_secs_f64() * 1000.0),
                )
                .field(
                    "face",
                    face_landmarks
                        .as_ref()
                        .map_or("none".to_string(), |v| v.len().to_string()),
                )
                .emit();

            return Ok(HolisticResult {
                face_landmarks,
                pose_landmarks_3d: None,
                pose_landmarks_2d: None,
                left_hand_landmarks: None,
                right_hand_landmarks: None,
            });
        }

        // Pose detection
        let (pose_3d, pose_2d) = match self.pose_detector.detect(frame) {
            Ok(result) => result,
            Err(e) => {
                pipeline_logger::tracker(log::Level::Warn, "pose detection error")
                    .field("error", format!("{e:#}"))
                    .emit();
                (None, None)
            }
        };

        let img_w = frame.width();
        let img_h = frame.height();

        // Use pose wrist landmarks to crop hand ROIs for better accuracy.
        // Pose landmark index 15 = left wrist, 16 = right wrist.
        let (left_frame, right_frame) = match &pose_2d {
            Some(landmarks) if landmarks.len() > 16 => {
                let left_wrist = landmarks[15];
                let right_wrist = landmarks[16];
                let (lx, ly, lw, lh) = calc_hand_roi(left_wrist, img_w, img_h);
                let (rx, ry, rw, rh) = calc_hand_roi(right_wrist, img_w, img_h);
                let left_crop = if lw > 0 && lh > 0 {
                    crop_image(frame, lx, ly, lw, lh)
                } else {
                    frame.clone()
                };
                let right_crop = if rw > 0 && rh > 0 {
                    crop_image(frame, rx, ry, rw, rh)
                } else {
                    frame.clone()
                };
                (left_crop, right_crop)
            }
            _ => (frame.clone(), frame.clone()),
        };

        let left_hand = match self.left_hand_detector.detect(&left_frame, true) {
            Ok(result) => result,
            Err(e) => {
                pipeline_logger::tracker(log::Level::Warn, "left hand detection error")
                    .field("error", format!("{e:#}"))
                    .emit();
                None
            }
        };
        let right_hand = match self.right_hand_detector.detect(&right_frame, false) {
            Ok(result) => result,
            Err(e) => {
                pipeline_logger::tracker(log::Level::Warn, "right hand detection error")
                    .field("error", format!("{e:#}"))
                    .emit();
                None
            }
        };

        let elapsed = detect_start.elapsed();
        pipeline_logger::tracker(log::Level::Debug, "detection complete")
            .field(
                "elapsed_ms",
                format!("{:.1}", elapsed.as_secs_f64() * 1000.0),
            )
            .field(
                "face",
                face_landmarks
                    .as_ref()
                    .map_or("none".to_string(), |v| v.len().to_string()),
            )
            .field(
                "pose_3d",
                pose_3d
                    .as_ref()
                    .map_or("none".to_string(), |v| v.len().to_string()),
            )
            .field(
                "left_hand",
                left_hand
                    .as_ref()
                    .map_or("none".to_string(), |v| v.len().to_string()),
            )
            .field(
                "right_hand",
                right_hand
                    .as_ref()
                    .map_or("none".to_string(), |v| v.len().to_string()),
            )
            .emit();

        Ok(HolisticResult {
            face_landmarks,
            pose_landmarks_3d: pose_3d,
            pose_landmarks_2d: pose_2d,
            left_hand_landmarks: left_hand,
            right_hand_landmarks: right_hand,
        })
    }
}
