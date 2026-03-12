use std::sync::Mutex;

use glam::Vec3;
use image::DynamicImage;
use log;
use ort::session::Session;
use ort::value::TensorRef;

use crate::preprocess;

/// Minimum face presence confidence to accept a detection.
const MIN_FACE_CONFIDENCE: f32 = 0.5;

/// Exponential moving average factor for landmark temporal smoothing.
/// Lower = smoother but more lag; higher = more responsive but jittery.
const LANDMARK_SMOOTHING: f32 = 0.6;

/// Exponential moving average factor for crop region smoothing.
const CROP_SMOOTHING: f32 = 0.3;

/// Crop margin as fraction of face size (each side).
const CROP_MARGIN: f32 = 0.25;

/// Face crop region in pixel coordinates.
#[derive(Debug, Clone, Copy)]
pub struct FaceCropRegion {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Face mesh detector using ONNX Runtime.
///
/// Detects 468 (or 478 with iris) face landmarks from an image.
/// Uses confidence-based filtering and temporal smoothing for stability.
pub struct FaceMeshDetector {
    session: Mutex<Session>,
    /// Previous frame's face crop region for tracking.
    prev_crop: Mutex<Option<FaceCropRegion>>,
    /// Previous frame's smoothed landmarks for temporal filtering.
    prev_landmarks: Mutex<Option<Vec<Vec3>>>,
    /// Number of consecutive frames without a confident detection.
    miss_count: Mutex<u32>,
}

impl FaceMeshDetector {
    /// Initialize from an ONNX model file.
    pub fn new(model_path: &str) -> anyhow::Result<Self> {
        let session = Session::builder()?.commit_from_file(model_path)?;
        Ok(Self {
            session: Mutex::new(session),
            prev_crop: Mutex::new(None),
            prev_landmarks: Mutex::new(None),
            miss_count: Mutex::new(0),
        })
    }

    /// Detect face landmarks with automatic face region tracking.
    ///
    /// Uses confidence scoring, crop tracking, and temporal smoothing.
    /// Landmarks are returned in normalized [0,1] coordinates relative to the FULL frame.
    pub fn detect(&self, frame: &DynamicImage) -> anyhow::Result<Option<Vec<Vec3>>> {
        let img_w = frame.width();
        let img_h = frame.height();

        // Determine crop region
        let crop = {
            let prev = self.prev_crop.lock().unwrap();
            match *prev {
                Some(c) => c,
                None => center_crop(img_w, img_h, 0.45),
            }
        };

        // Crop the frame
        let cropped = if crop.x == 0 && crop.y == 0 && crop.w == img_w && crop.h == img_h {
            frame.clone()
        } else {
            frame.crop_imm(crop.x, crop.y, crop.w, crop.h)
        };

        pipeline_logger::tracker(log::Level::Debug, "face crop region")
            .field("crop", format!("({},{}){}x{}", crop.x, crop.y, crop.w, crop.h))
            .field("frame", format!("{}x{}", img_w, img_h))
            .emit();

        // Run inference on cropped image
        let input_tensor = preprocess::preprocess_image(&cropped, 192, 192);
        let input_ref = TensorRef::from_array_view(&input_tensor)?;

        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs![input_ref])?;

        // Check face presence confidence (output[1]: shape [1,1,1,1])
        let confidence = if outputs.len() > 1 {
            let (_shape, conf_data) = outputs[1].try_extract_tensor::<f32>()?;
            let raw_conf = conf_data.first().copied().unwrap_or(0.0);
            // Apply sigmoid: the raw output is a logit
            1.0 / (1.0 + (-raw_conf).exp())
        } else {
            1.0 // No confidence output, assume confident
        };

        let (shape, raw_data) = outputs[0].try_extract_tensor::<f32>()?;

        pipeline_logger::tracker(log::Level::Debug, "face output")
            .field("shape", format!("{:?}", shape))
            .field("data_len", raw_data.len())
            .field("confidence", format!("{:.3}", confidence))
            .emit();

        // Reject low-confidence detections
        if confidence < MIN_FACE_CONFIDENCE {
            let mut miss = self.miss_count.lock().unwrap();
            *miss += 1;
            pipeline_logger::tracker(log::Level::Debug, "face: low confidence, skipping")
                .field("confidence", format!("{:.3}", confidence))
                .field("miss_count", *miss)
                .emit();

            // After many consecutive misses, reset crop to center
            if *miss > 30 {
                *self.prev_crop.lock().unwrap() = None;
                *self.prev_landmarks.lock().unwrap() = None;
                *miss = 0;
            }

            // Return previous landmarks if available (hold last good detection)
            return Ok(self.prev_landmarks.lock().unwrap().clone());
        }

        // Reset miss counter on good detection
        *self.miss_count.lock().unwrap() = 0;

        if raw_data.is_empty() {
            return Ok(None);
        }

        let num_landmarks = raw_data.len() / 3;
        if num_landmarks < 468 {
            pipeline_logger::tracker(log::Level::Warn, "face: insufficient landmarks")
                .field("num_landmarks", num_landmarks)
                .emit();
            return Ok(None);
        }

        // Normalize landmarks to [0,1] relative to the 192x192 input
        let crop_landmarks =
            preprocess::normalize_landmarks(raw_data, num_landmarks, 192.0, 192.0);

        // Map landmarks from crop-relative [0,1] to full-frame [0,1]
        let full_landmarks: Vec<Vec3> = crop_landmarks
            .iter()
            .map(|lm| {
                let full_x = (crop.x as f32 + lm.x * crop.w as f32) / img_w as f32;
                let full_y = (crop.y as f32 + lm.y * crop.h as f32) / img_h as f32;
                Vec3::new(full_x, full_y, lm.z)
            })
            .collect();

        // Apply temporal smoothing (EMA) to landmarks.
        // Eye lid landmarks use a higher alpha (less smoothing) to preserve fast blinks.
        let smoothed = {
            let mut prev = self.prev_landmarks.lock().unwrap();
            let result = match &*prev {
                Some(prev_lm) if prev_lm.len() == full_landmarks.len() => {
                    full_landmarks
                        .iter()
                        .zip(prev_lm.iter())
                        .enumerate()
                        .map(|(i, (new, old))| {
                            let alpha = if is_eye_lid_landmark(i) {
                                0.9 // fast response for blink detection
                            } else {
                                LANDMARK_SMOOTHING
                            };
                            Vec3::new(
                                old.x + (new.x - old.x) * alpha,
                                old.y + (new.y - old.y) * alpha,
                                old.z + (new.z - old.z) * alpha,
                            )
                        })
                        .collect()
                }
                _ => full_landmarks,
            };
            *prev = Some(result.clone());
            result
        };

        // Update crop for next frame based on smoothed landmarks
        let next_crop = estimate_face_crop(&smoothed, img_w, img_h);
        // Smooth the crop region transition
        let smoothed_crop = {
            let mut prev = self.prev_crop.lock().unwrap();
            let result = match *prev {
                Some(old) => smooth_crop(old, next_crop, CROP_SMOOTHING),
                None => next_crop,
            };
            *prev = Some(result);
            result
        };
        // Write back the smoothed crop (overwrite what we just set)
        *self.prev_crop.lock().unwrap() = Some(smoothed_crop);

        Ok(Some(smoothed))
    }
}

/// Center crop: take the center `ratio` portion of the image as a square.
fn center_crop(img_w: u32, img_h: u32, ratio: f32) -> FaceCropRegion {
    let size = (img_w.min(img_h) as f32 * ratio) as u32;
    let x = (img_w - size) / 2;
    let y = (img_h - size) / 2;
    FaceCropRegion {
        x,
        y,
        w: size,
        h: size,
    }
}

/// Smoothly interpolate between two crop regions.
fn smooth_crop(old: FaceCropRegion, new: FaceCropRegion, alpha: f32) -> FaceCropRegion {
    let lerp = |a: u32, b: u32| -> u32 {
        (a as f32 + (b as f32 - a as f32) * alpha).round().max(0.0) as u32
    };
    FaceCropRegion {
        x: lerp(old.x, new.x),
        y: lerp(old.y, new.y),
        w: lerp(old.w, new.w).max(1),
        h: lerp(old.h, new.h).max(1),
    }
}

/// Estimate face bounding box from landmarks for the next frame's crop.
///
/// Uses landmarks to find the face extent with margin for tracking stability.
fn estimate_face_crop(landmarks: &[Vec3], img_w: u32, img_h: u32) -> FaceCropRegion {
    if landmarks.is_empty() {
        return center_crop(img_w, img_h, 0.45);
    }

    // Find bounding box of all face landmarks (in normalized [0,1] coords)
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for lm in landmarks.iter().take(468) {
        min_x = min_x.min(lm.x);
        min_y = min_y.min(lm.y);
        max_x = max_x.max(lm.x);
        max_y = max_y.max(lm.y);
    }

    // Convert to pixel coordinates
    let face_x = min_x * img_w as f32;
    let face_y = min_y * img_h as f32;
    let face_w = (max_x - min_x) * img_w as f32;
    let face_h = (max_y - min_y) * img_h as f32;

    // Make square (use larger dimension) and add margin
    let face_size = face_w.max(face_h);
    let margin = face_size * CROP_MARGIN;
    let crop_size = (face_size + margin * 2.0) as u32;

    // Center the crop on the face center
    let cx = face_x + face_w * 0.5;
    let cy = face_y + face_h * 0.5;
    let half = crop_size as f32 * 0.5;

    let x = (cx - half).max(0.0) as u32;
    let y = (cy - half).max(0.0) as u32;
    let w = crop_size.min(img_w.saturating_sub(x));
    let h = crop_size.min(img_h.saturating_sub(y));

    // Ensure minimum crop size (at least 20% of smallest dimension)
    let min_size = (img_w.min(img_h) as f32 * 0.2) as u32;
    if w < min_size || h < min_size {
        return center_crop(img_w, img_h, 0.45);
    }

    FaceCropRegion { x, y, w, h }
}

/// Check if a landmark index is part of the eye lid (used for blink detection).
/// These landmarks need less smoothing to preserve fast blink movements.
fn is_eye_lid_landmark(index: usize) -> bool {
    // Left eye lid: 160,159,158 (upper), 144,145,153 (lower), 130,133 (corners)
    // Right eye lid: 387,386,385 (upper), 373,374,380 (lower), 263,362 (corners)
    matches!(
        index,
        130 | 133 | 144 | 145 | 153 | 158 | 159 | 160 | 263 | 362 | 373 | 374 | 380 | 385
            | 386 | 387
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_invalid_path_returns_error() {
        let result = FaceMeshDetector::new("/nonexistent/model.onnx");
        assert!(result.is_err());
    }

    #[test]
    fn center_crop_produces_valid_region() {
        let crop = center_crop(640, 480, 0.45);
        assert_eq!(crop.w, 216); // 480 * 0.45
        assert_eq!(crop.h, 216);
        assert!(crop.x + crop.w <= 640);
        assert!(crop.y + crop.h <= 480);
    }

    #[test]
    fn estimate_crop_with_landmarks() {
        let landmarks: Vec<Vec3> = (0..468)
            .map(|_| Vec3::new(0.4, 0.3, 0.0))
            .chain((0..468).map(|_| Vec3::new(0.6, 0.7, 0.0)))
            .collect();
        let crop = estimate_face_crop(&landmarks[..468], 640, 480);
        assert!(crop.x + crop.w <= 640);
        assert!(crop.y + crop.h <= 480);
    }

    #[test]
    fn smooth_crop_interpolates() {
        let old = FaceCropRegion {
            x: 100,
            y: 100,
            w: 200,
            h: 200,
        };
        let new = FaceCropRegion {
            x: 200,
            y: 200,
            w: 300,
            h: 300,
        };
        let result = smooth_crop(old, new, 0.5);
        assert_eq!(result.x, 150);
        assert_eq!(result.y, 150);
        assert_eq!(result.w, 250);
        assert_eq!(result.h, 250);
    }

    #[test]
    fn smooth_crop_alpha_zero_keeps_old() {
        let old = FaceCropRegion {
            x: 100,
            y: 100,
            w: 200,
            h: 200,
        };
        let new = FaceCropRegion {
            x: 300,
            y: 300,
            w: 400,
            h: 400,
        };
        let result = smooth_crop(old, new, 0.0);
        assert_eq!(result.x, old.x);
        assert_eq!(result.y, old.y);
    }
}
