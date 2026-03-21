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
pub fn mel_filterbank(n_mels: usize, n_fft: usize, sr: u32, fmin: f32, fmax: f32) -> Vec<Vec<f32>> {
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

/// PCM f32 → log-mel spectrogram.
///
/// Processing:
/// 1. STFT (Hann window, n_fft=400, hop=160)
/// 2. Apply mel filterbank
/// 3. log10 transform + clamp (max - 8.0 dB)
/// 4. Whisper-style normalization: (x - max) / 4.0 + 1.0
///
/// Output: `Vec<f32>` row-major (n_mels, n_frames) = (80, 800)
pub fn log_mel_spectrogram(audio: &[f32], config: &MelConfig) -> Vec<f32> {
    use crate::stft::{hann_window, stft_power};

    let n_frames_target =
        (config.chunk_length * config.sample_rate as f32 / config.hop_length as f32) as usize;
    let n_mels = config.n_mels;
    let n_fft = config.n_fft;
    let hop = config.hop_length;
    let n_bins = n_fft / 2 + 1;

    // Build mel filterbank: shape (n_mels, n_bins)
    let filters = mel_filterbank(n_mels, n_fft, config.sample_rate, config.fmin, config.fmax);

    // Compute STFT power spectrum
    let window = hann_window(n_fft);
    let power_frames = stft_power(audio, n_fft, hop, &window);
    // power_frames: Vec<Vec<f32>> shape (actual_frames, n_bins)

    let actual_frames = power_frames.len();

    // Apply mel filterbank: result shape (n_mels, n_frames_target) in row-major order
    let mut mel = vec![0.0_f32; n_mels * n_frames_target];

    for m in 0..n_mels {
        for t in 0..n_frames_target {
            if t < actual_frames {
                let mut sum = 0.0_f32;
                for k in 0..n_bins {
                    sum += filters[m][k] * power_frames[t][k];
                }
                mel[m * n_frames_target + t] = sum;
            }
            // else remains 0.0 (zero-padded)
        }
    }

    // log10 transform: clamp minimum to avoid log(0)
    let floor = 1e-10_f32;
    for val in mel.iter_mut() {
        *val = (*val).max(floor).log10();
    }

    // Clamp to (max - 8.0)
    let log_max = mel.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let clamp_min = log_max - 8.0;
    for val in mel.iter_mut() {
        *val = (*val).max(clamp_min);
    }

    // Whisper-style normalization: (x - max) / 4.0 + 1.0
    for val in mel.iter_mut() {
        *val = (*val - log_max) / 4.0 + 1.0;
    }

    mel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hz_to_mel_known_values() {
        // 80 Hz
        let mel_80 = hz_to_mel(80.0);
        assert!(
            (mel_80 - 121.956).abs() < 1.0,
            "80 Hz => ~122 mel, got {mel_80}"
        );

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
                assert!(val >= 0.0, "negative value {val} at mel={m}, bin={k}");
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

    // --- log_mel_spectrogram tests ---

    #[test]
    fn test_log_mel_silence() {
        let config = MelConfig::default();
        let audio = vec![0.0_f32; 128000]; // 8 seconds at 16kHz
        let mel = log_mel_spectrogram(&audio, &config);

        assert_eq!(
            mel.len(),
            64000,
            "expected 80*800=64000 elements, got {}",
            mel.len()
        );

        // All-zero input → all values should be identical after normalization
        let first = mel[0];
        for (i, &v) in mel.iter().enumerate() {
            assert!(
                (v - first).abs() < 1e-6,
                "silence: mel[{}] = {}, expected uniform value {}",
                i,
                v,
                first
            );
        }
    }

    #[test]
    fn test_log_mel_output_shape() {
        let config = MelConfig::default();
        let audio = vec![0.0_f32; 128000];
        let mel = log_mel_spectrogram(&audio, &config);
        assert_eq!(
            mel.len(),
            64000,
            "expected 80*800=64000 elements, got {}",
            mel.len()
        );
    }

    #[test]
    fn test_log_mel_non_nan() {
        let config = MelConfig::default();
        // Use a sine wave to get non-trivial values
        let audio: Vec<f32> = (0..128000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();
        let mel = log_mel_spectrogram(&audio, &config);

        for (i, &v) in mel.iter().enumerate() {
            assert!(!v.is_nan(), "NaN at index {}", i);
            assert!(!v.is_infinite(), "Inf at index {}", i);
        }
    }

    #[test]
    fn test_log_mel_range() {
        let config = MelConfig::default();
        let audio: Vec<f32> = (0..128000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();
        let mel = log_mel_spectrogram(&audio, &config);

        for (i, &v) in mel.iter().enumerate() {
            assert!(
                v >= -1.0 - 0.01 && v <= 1.0 + 0.01,
                "mel[{}] = {} out of expected range [-1, 1]",
                i,
                v
            );
        }
    }

    #[test]
    fn test_log_mel_short_audio() {
        let config = MelConfig::default();
        let audio = vec![0.1_f32; 160]; // Very short, shorter than n_fft
        let mel = log_mel_spectrogram(&audio, &config);

        // Should still produce the correct shape (zero-padded STFT frames)
        assert_eq!(
            mel.len(),
            64000,
            "expected 80*800=64000 elements even for short audio, got {}",
            mel.len()
        );

        // Should contain no NaN or Inf
        for (i, &v) in mel.iter().enumerate() {
            assert!(!v.is_nan(), "NaN at index {}", i);
            assert!(!v.is_infinite(), "Inf at index {}", i);
        }
    }
}
