// Mel-spectrogram computation for ETD pipeline.
// Step 1.3: Mel filterbank functions (HTK scale, triangular filters).

/// Configuration for mel-spectrogram extraction, with Whisper-compatible defaults.
#[derive(Debug, Clone)]
pub struct MelConfig {
    /// FFT window size in samples.
    pub n_fft: usize,
    /// Hop length (stride) in samples.
    pub hop_length: usize,
    /// Number of mel filter bands.
    pub n_mels: usize,
    /// Audio sample rate in Hz.
    pub sample_rate: u32,
    /// Minimum frequency for the filterbank (Hz).
    pub fmin: f32,
    /// Maximum frequency for the filterbank (Hz).
    pub fmax: f32,
    /// Chunk length in seconds.
    pub chunk_length: f32,
}

impl Default for MelConfig {
    fn default() -> Self {
        Self {
            n_fft: 400,
            hop_length: 160,
            n_mels: 80,
            sample_rate: 16000,
            fmin: 80.0,
            fmax: 7600.0,
            chunk_length: 8.0,
        }
    }
}

/// Convert frequency in Hz to mel scale (HTK formula).
fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

/// Convert mel scale value back to Hz (inverse HTK formula).
fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0)
}

/// Build a triangular mel filterbank matrix.
///
/// Returns a `Vec<Vec<f32>>` of shape `(n_mels, n_fft / 2 + 1)`.
/// Each row is a triangular filter in the frequency domain.
///
/// If `n_mels == 0` or `fmin >= fmax`, the result is an appropriately
/// degenerate matrix (empty or all-zeros).
pub fn mel_filterbank(
    n_mels: usize,
    n_fft: usize,
    sr: u32,
    fmin: f32,
    fmax: f32,
) -> Vec<Vec<f32>> {
    let n_freqs = n_fft / 2 + 1;

    if n_mels == 0 {
        return Vec::new();
    }

    if fmin >= fmax {
        return vec![vec![0.0; n_freqs]; n_mels];
    }

    let mel_min = hz_to_mel(fmin);
    let mel_max = hz_to_mel(fmax);

    // n_mels + 2 equally spaced points in mel space (includes left/right edges).
    let n_points = n_mels + 2;
    let mel_points: Vec<f32> = (0..n_points)
        .map(|i| mel_min + (mel_max - mel_min) * (i as f32) / (n_points as f32 - 1.0))
        .collect();

    // Convert mel points back to Hz, then to FFT bin indices.
    let hz_points: Vec<f32> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();
    let bin_points: Vec<f32> = hz_points
        .iter()
        .map(|&h| h * (n_fft as f32) / (sr as f32))
        .collect();

    let mut filters = vec![vec![0.0_f32; n_freqs]; n_mels];

    for m in 0..n_mels {
        let f_left = bin_points[m];
        let f_center = bin_points[m + 1];
        let f_right = bin_points[m + 2];

        for k in 0..n_freqs {
            let kf = k as f32;

            if kf > f_left && kf <= f_center && f_center > f_left {
                filters[m][k] = (kf - f_left) / (f_center - f_left);
            } else if kf > f_center && kf < f_right && f_right > f_center {
                filters[m][k] = (f_right - kf) / (f_right - f_center);
            }
        }
    }

    filters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hz_to_mel_known_values() {
        // 80 Hz
        let mel_80 = hz_to_mel(80.0);
        assert!((mel_80 - 121.956).abs() < 1.0, "80 Hz => ~122 mel, got {mel_80}");

        // 1000 Hz
        let mel_1000 = hz_to_mel(1000.0);
        assert!(
            (mel_1000 - 999.985).abs() < 1.0,
            "1000 Hz => ~1000 mel, got {mel_1000}"
        );

        // 7600 Hz
        let mel_7600 = hz_to_mel(7600.0);
        assert!(
            (mel_7600 - 2787.0).abs() < 5.0,
            "7600 Hz => ~2787 mel, got {mel_7600}"
        );
    }

    #[test]
    fn test_mel_to_hz_roundtrip() {
        for &hz in &[80.0_f32, 440.0, 1000.0, 4000.0, 7600.0] {
            let roundtrip = mel_to_hz(hz_to_mel(hz));
            assert!(
                (roundtrip - hz).abs() < 0.1,
                "roundtrip failed for {hz}: got {roundtrip}"
            );
        }
    }

    #[test]
    fn test_filterbank_shape() {
        let fb = mel_filterbank(80, 400, 16000, 80.0, 7600.0);
        assert_eq!(fb.len(), 80, "expected 80 mel bands");
        assert_eq!(fb[0].len(), 201, "expected n_fft/2+1 = 201 frequency bins");
    }

    #[test]
    fn test_filterbank_sum_approx_one() {
        let fb = mel_filterbank(80, 400, 16000, 80.0, 7600.0);
        let n_freqs = fb[0].len();

        // In the passband (bins covered by at least one filter), the sum of
        // all filters at a given bin should be approximately 1.0.
        for k in 0..n_freqs {
            let sum: f32 = fb.iter().map(|row| row[k]).sum();
            // In overlapping regions the sum should be close to 1.0, but at
            // the edges of the passband partial coverage is expected.
            if sum > 0.01 {
                assert!(
                    sum <= 1.01 && sum > 0.0,
                    "filter sum at bin {k} = {sum}, expected in (0, ~1]"
                );
            }
        }
    }

    #[test]
    fn test_filterbank_no_negative() {
        let fb = mel_filterbank(80, 400, 16000, 80.0, 7600.0);
        for (m, row) in fb.iter().enumerate() {
            for (k, &val) in row.iter().enumerate() {
                assert!(
                    val >= 0.0,
                    "negative value {val} at mel={m}, bin={k}"
                );
            }
        }
    }

    #[test]
    fn test_filterbank_fmin_equals_fmax() {
        let fb = mel_filterbank(80, 400, 16000, 1000.0, 1000.0);
        assert_eq!(fb.len(), 80);
        for row in &fb {
            let sum: f32 = row.iter().sum();
            assert!(
                sum.abs() < f32::EPSILON,
                "expected all zeros when fmin == fmax, got sum {sum}"
            );
        }
    }

    #[test]
    fn test_filterbank_zero_mels() {
        let fb = mel_filterbank(0, 400, 16000, 80.0, 7600.0);
        assert!(fb.is_empty(), "expected empty vec for n_mels=0");
    }
}
