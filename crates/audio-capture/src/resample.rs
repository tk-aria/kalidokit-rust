/// Average channels to mono.
pub fn downmix_to_mono(data: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return data.to_vec();
    }
    data.chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Nearest-neighbor resample.
pub fn resample_nearest(data: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if src_rate == dst_rate {
        return data.to_vec();
    }
    let ratio = src_rate as f64 / dst_rate as f64;
    let out_len = (data.len() as f64 / ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);
    let mut pos = 0.0f64;
    while (pos as usize) < data.len() {
        out.push(data[pos as usize]);
        pos += ratio;
    }
    out
}

/// Convert f32 [-1.0, 1.0] to i16.
pub fn f32_to_i16(data: &[f32]) -> Vec<i16> {
    data.iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_passthrough() {
        let data = vec![0.5, -0.5, 0.3];
        assert_eq!(downmix_to_mono(&data, 1), data);
    }

    #[test]
    fn stereo_to_mono() {
        let stereo = vec![0.4, 0.6, -0.2, 0.2];
        let mono = downmix_to_mono(&stereo, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.5).abs() < 1e-6);
        assert!((mono[1] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn resample_same_rate() {
        let data = vec![1.0, 2.0, 3.0];
        assert_eq!(resample_nearest(&data, 16000, 16000), data);
    }

    #[test]
    fn resample_downsample() {
        // 48kHz to 16kHz = ratio 3, so output is ~1/3 of input
        let data: Vec<f32> = (0..300).map(|i| i as f32 / 300.0).collect();
        let out = resample_nearest(&data, 48000, 16000);
        assert!(out.len() >= 99 && out.len() <= 101);
    }

    #[test]
    fn f32_to_i16_clipping() {
        let data = vec![0.0, 1.0, -1.0, 1.5, -1.5];
        let i16s = f32_to_i16(&data);
        assert_eq!(i16s[0], 0);
        assert_eq!(i16s[1], 32767);
        assert_eq!(i16s[2], -32767);
        assert_eq!(i16s[3], 32767); // clipped
        assert_eq!(i16s[4], -32767); // clipped
    }
}
