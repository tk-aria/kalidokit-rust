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
        image::imageops::FilterType::Bilinear,
    );
    let rgb = resized.to_rgb8();

    let mut tensor =
        Array4::<f32>::zeros((1, 3, target_height as usize, target_width as usize));
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
