use glam::Vec3;
use image::DynamicImage;
use ndarray::Array4;

/// Convert an image to a model input tensor [1, 3, H, W] normalized to 0-1.
pub fn preprocess_image(
    image: &DynamicImage,
    target_width: u32,
    target_height: u32,
) -> Array4<f32> {
    let resized = image.resize_exact(
        target_width,
        target_height,
        image::imageops::FilterType::Triangle,
    );
    let rgb = resized.to_rgb8();

    let mut tensor = Array4::<f32>::zeros((1, 3, target_height as usize, target_width as usize));
    for y in 0..target_height {
        for x in 0..target_width {
            let pixel = rgb.get_pixel(x, y);
            tensor[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
            tensor[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
            tensor[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
        }
    }
    tensor
}

/// Normalize raw model output landmarks to 0-1 range relative to image dimensions.
///
/// Input: flat array of [x, y, z, ...] values in pixel coordinates.
/// Output: Vec<Vec3> with x/y normalized to 0-1 and z preserved.
pub fn normalize_landmarks(
    raw_output: &[f32],
    num_landmarks: usize,
    image_width: f32,
    image_height: f32,
) -> Vec<Vec3> {
    let stride = if raw_output.len() >= num_landmarks * 3 {
        raw_output.len() / num_landmarks
    } else {
        return Vec::new();
    };

    (0..num_landmarks)
        .map(|i| {
            let offset = i * stride;
            let x = raw_output[offset] / image_width;
            let y = raw_output[offset + 1] / image_height;
            let z = if stride >= 3 {
                raw_output[offset + 2]
            } else {
                0.0
            };
            Vec3::new(x, y, z)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocess_image_shape() {
        let img = image::DynamicImage::new_rgb8(640, 480);
        let tensor = preprocess_image(&img, 192, 192);
        assert_eq!(tensor.shape(), &[1, 3, 192, 192]);
    }

    #[test]
    fn preprocess_image_values_in_range() {
        let img = image::DynamicImage::new_rgb8(64, 48);
        let tensor = preprocess_image(&img, 32, 32);
        for &v in tensor.iter() {
            assert!(v >= 0.0 && v <= 1.0, "value out of range: {}", v);
        }
    }

    #[test]
    fn preprocess_zero_size_image() {
        let img = image::DynamicImage::new_rgb8(1, 1);
        let tensor = preprocess_image(&img, 4, 4);
        assert_eq!(tensor.shape(), &[1, 3, 4, 4]);
    }

    #[test]
    fn normalize_landmarks_basic() {
        // 3 landmarks with x,y,z
        let raw = vec![100.0, 200.0, 0.5, 50.0, 100.0, 0.3, 150.0, 300.0, 0.8];
        let result = normalize_landmarks(&raw, 3, 200.0, 400.0);
        assert_eq!(result.len(), 3);
        assert!((result[0].x - 0.5).abs() < 1e-6);
        assert!((result[0].y - 0.5).abs() < 1e-6);
        assert!((result[0].z - 0.5).abs() < 1e-6);
    }

    #[test]
    fn normalize_landmarks_count_matches() {
        let raw = vec![0.0; 468 * 3];
        let result = normalize_landmarks(&raw, 468, 192.0, 192.0);
        assert_eq!(result.len(), 468);
    }

    #[test]
    fn normalize_landmarks_empty_input() {
        let result = normalize_landmarks(&[], 5, 100.0, 100.0);
        assert!(result.is_empty());
    }
}
