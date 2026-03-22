use crate::{
    face_mesh::FaceMeshDetector,
    hand::HandDetector,
    palm::{PalmDetector, palm_to_hand_roi},
    pose::PoseDetector,
    preprocess::crop_image,
    HolisticResult,
};
use image::DynamicImage;
use log;

/// Palm detection model path (PINTO0309 post-processed model).
const DEFAULT_PALM_MODEL: &str = "assets/models/palm_detection_post.onnx";

/// Combined inference pipeline for face, pose, and hand tracking.
///
/// Hand detection uses a 2-stage pipeline matching MediaPipe:
/// 1. Palm detection (BlazePalm) → hand bounding boxes
/// 2. Hand landmark model → 21 landmarks per detected hand
pub struct HolisticTracker {
    face_detector: FaceMeshDetector,
    pose_detector: PoseDetector,
    palm_detector: Option<PalmDetector>,
    hand_detector: HandDetector,
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
        let hand_detector = HandDetector::new(hand_model_path)?;

        // Palm detector is optional — hand tracking disabled if model not found
        let palm_detector = match PalmDetector::new(DEFAULT_PALM_MODEL) {
            Ok(pd) => {
                log::info!("Palm detection model loaded: {DEFAULT_PALM_MODEL}");
                Some(pd)
            }
            Err(e) => {
                log::warn!("Palm detection model not found ({DEFAULT_PALM_MODEL}): {e}. Hand tracking disabled.");
                None
            }
        };

        Ok(Self {
            face_detector,
            pose_detector,
            palm_detector,
            hand_detector,
        })
    }

    /// Detect all landmarks from a camera frame.
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
                .emit();

            return Ok(HolisticResult {
                face_landmarks,
                pose_landmarks_3d: None,
                pose_landmarks_2d: None,
                left_hand_landmarks: None,
                right_hand_landmarks: None,
            });
        }

        // Pose detection (needed for body tracking, optional for hand)
        let (pose_3d, pose_2d) = match self.pose_detector.detect(frame) {
            Ok(result) => result,
            Err(e) => {
                pipeline_logger::tracker(log::Level::Warn, "pose detection error")
                    .field("error", format!("{e:#}"))
                    .emit();
                (None, None)
            }
        };

        // Hand detection: 2-stage pipeline (palm detection → hand landmark)
        let img_w = frame.width();
        let img_h = frame.height();

        let (left_hand, right_hand) = if let Some(palm_det) = &self.palm_detector {
            self.detect_hands_via_palm(frame, palm_det, img_w, img_h)
        } else {
            (None, None)
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

    /// 2-stage hand detection: palm detection → crop → hand landmark.
    ///
    /// Detects up to 2 hands. Assigns left/right based on x-position
    /// (camera-mirrored: left side of image = user's right hand).
    fn detect_hands_via_palm(
        &self,
        frame: &DynamicImage,
        palm_det: &PalmDetector,
        img_w: u32,
        img_h: u32,
    ) -> (Option<Vec<glam::Vec3>>, Option<Vec<glam::Vec3>>) {
        let palms = match palm_det.detect(frame, img_w, img_h) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Palm detection error: {e}");
                return (None, None);
            }
        };

        log::trace!("Palm detections: {} palms found", palms.len());
        if palms.is_empty() {
            return (None, None);
        }

        let img_w = frame.width();
        let img_h = frame.height();
        let mut left_hand = None;
        let mut right_hand = None;

        // Process up to 2 best detections
        for palm in palms.iter().take(2) {
            let (rx, ry, rw, rh) = palm_to_hand_roi(palm, img_w, img_h);
            if rw == 0 || rh == 0 {
                continue;
            }

            // Determine handedness by position:
            // Camera mirror: palm on left side of image (cx < 0.5) = user's right hand
            let is_left = palm.cx >= 0.5; // camera-mirrored
            log::trace!("  palm[cx={:.3},cy={:.3},score={:.3}] → ROI({},{},{},{}) is_left={}", palm.cx, palm.cy, palm.score, rx, ry, rw, rh, is_left);
            let crop = crop_image(frame, rx, ry, rw, rh);

            let landmarks = match self.hand_detector.detect(&crop, is_left) {
                Ok(Some(lm)) => {
                    log::debug!("Hand landmarks detected: {} points", lm.len());
                    Some(lm)
                }
                Ok(None) => None,
                Err(e) => {
                    log::warn!("Hand landmark error: {e}");
                    None
                }
            };

            if let Some(lm) = landmarks {
                if is_left {
                    left_hand = Some(lm);
                } else {
                    right_hand = Some(lm);
                }
            }
        }

        (left_hand, right_hand)
    }
}
