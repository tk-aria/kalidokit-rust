// Audio input handling for ETD pipeline.

/// Convert i16 PCM samples to f32 in the range [-1.0, 1.0].
pub fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / 32768.0).collect()
}

/// Truncate audio to the last `max_seconds` worth of samples, or pad with
/// leading zeros if shorter (smart-turn spec: the model always receives a
/// fixed-length window aligned to the *end* of the utterance).
pub fn truncate_or_pad(audio: &[f32], sample_rate: u32, max_seconds: f32) -> Vec<f32> {
    let max_samples = (sample_rate as f32 * max_seconds) as usize;
    let len = audio.len();

    if len >= max_samples {
        // Take the last max_samples
        audio[len - max_samples..].to_vec()
    } else {
        // Pad with zeros at the beginning
        let mut padded = vec![0.0f32; max_samples - len];
        padded.extend_from_slice(audio);
        padded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- i16_to_f32 normal cases ----

    #[test]
    fn test_i16_to_f32_zero() {
        let result = i16_to_f32(&[0i16]);
        assert_eq!(result, vec![0.0f32]);
    }

    #[test]
    fn test_i16_to_f32_max() {
        let result = i16_to_f32(&[32767i16]);
        let expected = 32767.0f32 / 32768.0;
        assert!(
            (result[0] - expected).abs() < 1e-6,
            "expected ~1.0, got {}",
            result[0]
        );
    }

    #[test]
    fn test_i16_to_f32_min() {
        let result = i16_to_f32(&[-32768i16]);
        assert_eq!(result[0], -1.0f32);
    }

    // ---- truncate_or_pad normal cases (sample_rate=16000, max_seconds=8.0 → 128000) ----

    #[test]
    fn test_truncate_exact_8s() {
        let audio = vec![0.5f32; 128_000];
        let result = truncate_or_pad(&audio, 16_000, 8.0);
        assert_eq!(result.len(), 128_000);
        assert_eq!(result, audio);
    }

    #[test]
    fn test_truncate_longer() {
        let mut audio = vec![0.0f32; 64_000];
        audio.extend(vec![1.0f32; 128_000]);
        assert_eq!(audio.len(), 192_000);

        let result = truncate_or_pad(&audio, 16_000, 8.0);
        assert_eq!(result.len(), 128_000);
        // Should keep the last 128000 samples (all 1.0)
        assert!(result.iter().all(|&v| v == 1.0));
    }

    #[test]
    fn test_pad_shorter() {
        let audio = vec![1.0f32; 64_000];
        let result = truncate_or_pad(&audio, 16_000, 8.0);
        assert_eq!(result.len(), 128_000);
        // First 64000 should be zeros (padding)
        assert!(result[..64_000].iter().all(|&v| v == 0.0));
        // Last 64000 should be the original data
        assert!(result[64_000..].iter().all(|&v| v == 1.0));
    }

    #[test]
    fn test_pad_empty() {
        let audio: Vec<f32> = vec![];
        let result = truncate_or_pad(&audio, 16_000, 8.0);
        assert_eq!(result.len(), 128_000);
        assert!(result.iter().all(|&v| v == 0.0));
    }

    // ---- edge / error cases ----

    #[test]
    fn test_truncate_zero_max_seconds() {
        let audio = vec![1.0f32; 1000];
        let result = truncate_or_pad(&audio, 16_000, 0.0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_i16_to_f32_empty() {
        let result = i16_to_f32(&[]);
        assert!(result.is_empty());
    }
}
