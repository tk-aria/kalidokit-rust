//! Palm detection using PINTO0309's post-processed BlazePalm ONNX model.
//!
//! Model: palm_detection_full_inf_post_192x192.onnx
//! Input: [1, 3, 192, 192] NCHW, float32, normalized 0-1, RGB
//! Output: [N, 8] — pd_score, box_x, box_y, box_size, kp0_x, kp0_y, kp2_x, kp2_y
//!
//! Post-processing (anchors, NMS) is already baked into the model.

use std::sync::Mutex;

use image::DynamicImage;
use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;

/// Minimum confidence to accept a palm detection.
const PALM_SCORE_THRESHOLD: f32 = 0.5;

/// A detected palm with bounding box and rotation.
#[derive(Debug, Clone)]
pub struct PalmDetection {
    /// Normalized center x (0-1).
    pub cx: f32,
    /// Normalized center y (0-1).
    pub cy: f32,
    /// Normalized bounding box size (larger of w/h).
    pub size: f32,
    /// Rotation in radians (from wrist→middle finger direction).
    pub rotation: f32,
    /// Confidence score.
    pub score: f32,
}

/// Palm detector using PINTO0309's post-processed ONNX model.
pub struct PalmDetector {
    session: Mutex<Session>,
}

impl PalmDetector {
    pub fn new(model_path: &str) -> anyhow::Result<Self> {
        let session = Session::builder()?.commit_from_file(model_path)?;
        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Detect palms in the frame.
    pub fn detect(
        &self,
        frame: &DynamicImage,
        image_width: u32,
        image_height: u32,
    ) -> anyhow::Result<Vec<PalmDetection>> {
        // Preprocess: resize to 192x192, normalize to 0-1, RGB, NCHW
        let input_tensor = preprocess_nchw(frame, 192, 192);
        let input_ref = TensorRef::from_array_view(&input_tensor)?;

        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs![input_ref])?;

        if outputs.len() == 0 {
            return Ok(Vec::new());
        }

        // Output: [N, 8] — pd_score, box_x, box_y, box_size, kp0_x, kp0_y, kp2_x, kp2_y
        let (_, raw_data) = outputs[0].try_extract_tensor::<f32>()?;
        let num_detections = raw_data.len() / 8;

        // Compute padding used during preprocessing for coordinate un-mapping.
        // preprocess_nchw does: aspect-ratio resize to fit 192x192, then center-pad.
        let square_size = image_width.max(image_height) as f32;
        let scale = 192.0 / square_size;
        let resized_w = (image_width as f32 * scale).round();
        let resized_h = (image_height as f32 * scale).round();
        let pad_x = (192.0 - resized_w) / 2.0;
        let pad_y = (192.0 - resized_h) / 2.0;

        log::trace!("Palm detection: {} raw detections, pad=({:.1},{:.1}) resized=({:.0},{:.0})",
            num_detections, pad_x, pad_y, resized_w, resized_h);

        let mut detections = Vec::new();
        for i in 0..num_detections {
            let offset = i * 8;
            let pd_score = raw_data[offset];
            let box_x = raw_data[offset + 1];
            let box_y = raw_data[offset + 2];
            let box_size = raw_data[offset + 3];
            let kp0_x = raw_data[offset + 4];
            let kp0_y = raw_data[offset + 5];
            let kp2_x = raw_data[offset + 6];
            let kp2_y = raw_data[offset + 7];

            if pd_score < PALM_SCORE_THRESHOLD || box_size <= 0.0 {
                continue;
            }

            // Compute rotation from keypoints (wrist kp0 → middle finger kp2)
            let kp02_x = kp2_x - kp0_x;
            let kp02_y = kp2_y - kp0_y;
            let rotation = std::f32::consts::FRAC_PI_2 - kp02_y.atan2(kp02_x);
            let rotation = normalize_radians(rotation);

            // Compute rotated rect center (in padded-square normalized coords)
            let sqn_rr_size = 2.9 * box_size;
            let sqn_rr_center_x = box_x + 0.5 * box_size * rotation.sin();
            let sqn_rr_center_y = box_y - 0.5 * box_size * rotation.cos();

            // Transform from padded-192x192 coords to original image coords:
            // 1. Convert normalized [0,1] → pixel in 192x192 padded space
            // 2. Remove padding offset
            // 3. Convert to original image normalized coords
            let cx = ((sqn_rr_center_x * 192.0 - pad_x) / resized_w).clamp(0.0, 1.0);
            let cy = ((sqn_rr_center_y * 192.0 - pad_y) / resized_h).clamp(0.0, 1.0);

            detections.push(PalmDetection {
                cx,
                cy,
                size: sqn_rr_size,
                rotation,
                score: pd_score,
            });
        }

        Ok(detections)
    }
}

/// Preprocess image for NCHW input: resize, normalize 0-1, RGB, channels first.
fn preprocess_nchw(image: &DynamicImage, target_w: u32, target_h: u32) -> Array4<f32> {
    let rgb = image.to_rgb8();
    let img_w = rgb.width();
    let img_h = rgb.height();

    // Aspect-ratio preserving resize with padding
    let scale = (target_w as f32 / img_w as f32).min(target_h as f32 / img_h as f32);
    let new_w = (img_w as f32 * scale).round() as u32;
    let new_h = (img_h as f32 * scale).round() as u32;

    let resized = image::imageops::resize(&rgb, new_w, new_h, image::imageops::FilterType::Triangle);

    let pad_x = (target_w - new_w) / 2;
    let pad_y = (target_h - new_h) / 2;

    let mut tensor = Array4::<f32>::zeros((1, 3, target_h as usize, target_w as usize));

    for y in 0..new_h {
        for x in 0..new_w {
            let pixel = resized.get_pixel(x, y);
            let ty = (y + pad_y) as usize;
            let tx = (x + pad_x) as usize;
            if ty < target_h as usize && tx < target_w as usize {
                // Model expects BGR (trained with OpenCV BGR input)
                tensor[[0, 0, ty, tx]] = pixel[2] as f32 / 255.0; // B
                tensor[[0, 1, ty, tx]] = pixel[1] as f32 / 255.0; // G
                tensor[[0, 2, ty, tx]] = pixel[0] as f32 / 255.0; // R
            }
        }
    }

    tensor
}

fn normalize_radians(angle: f32) -> f32 {
    let mut a = angle % (2.0 * std::f32::consts::PI);
    if a < -std::f32::consts::PI {
        a += 2.0 * std::f32::consts::PI;
    } else if a > std::f32::consts::PI {
        a -= 2.0 * std::f32::consts::PI;
    }
    a
}

/// Convert a palm detection to a hand ROI for landmark inference.
/// Returns (x, y, w, h) in pixel coordinates.
pub fn palm_to_hand_roi(
    palm: &PalmDetection,
    img_w: u32,
    img_h: u32,
) -> (u32, u32, u32, u32) {
    let roi_size_px = (palm.size * img_w.max(img_h) as f32).round() as u32;
    let roi_size = roi_size_px.max(10); // minimum 10px

    let cx = (palm.cx * img_w as f32).round() as i32;
    let cy = (palm.cy * img_h as f32).round() as i32;
    let half = roi_size as i32 / 2;

    let x = (cx - half).max(0) as u32;
    let y = (cy - half).max(0) as u32;
    let w = roi_size.min(img_w.saturating_sub(x));
    let h = roi_size.min(img_h.saturating_sub(y));

    (x, y, w, h)
}
