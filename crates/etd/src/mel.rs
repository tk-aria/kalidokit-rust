// Mel-spectrogram computation for ETD pipeline.
// Whisper-compatible: slaney mel scale, slaney normalization, fmin=0, fmax=8000.

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
            fmin: 0.0,
            fmax: 8000.0,
            chunk_length: 8.0,
        }
    }
}

// --- Slaney mel scale ---
// Below 1000 Hz: linear (3 * hz / 200)
// Above 1000 Hz: logarithmic

#[cfg(test)]
const MIN_LOG_HERTZ: f32 = 1000.0;
#[cfg(test)]
const MIN_LOG_MEL: f32 = 15.0; // 3 * 1000 / 200
#[cfg(test)]
const LOGSTEP: f32 = 14.500_775; // 27.0 / ln(6.4)

#[cfg(test)]
fn hz_to_mel_slaney(hz: f32) -> f32 {
    if hz < MIN_LOG_HERTZ {
        3.0 * hz / 200.0
    } else {
        MIN_LOG_MEL + (hz / MIN_LOG_HERTZ).ln() * LOGSTEP
    }
}

#[cfg(test)]
fn mel_to_hz_slaney(mel: f32) -> f32 {
    if mel < MIN_LOG_MEL {
        200.0 * mel / 3.0
    } else {
        MIN_LOG_HERTZ * ((mel - MIN_LOG_MEL) / LOGSTEP).exp()
    }
}

/// Build a Whisper-compatible mel filterbank (slaney scale + slaney normalization).
///
/// Computed in f64 precision to match Python's `mel_filter_bank()` which uses float64.
/// Returns `Vec<Vec<f64>>` of shape `(n_mels, n_fft / 2 + 1)`.
pub fn mel_filterbank(n_mels: usize, n_fft: usize, sr: u32, fmin: f32, fmax: f32) -> Vec<Vec<f64>> {
    let n_freqs = n_fft / 2 + 1;

    if n_mels == 0 {
        return Vec::new();
    }
    if fmin >= fmax {
        return vec![vec![0.0; n_freqs]; n_mels];
    }

    // Use f64 for mel scale conversion to match Python precision
    let fmin64 = fmin as f64;
    let fmax64 = fmax as f64;

    let min_log_hertz: f64 = 1000.0;
    let min_log_mel: f64 = 15.0;
    let logstep: f64 = 27.0 / (6.4_f64).ln();

    let hz_to_mel = |hz: f64| -> f64 {
        if hz < min_log_hertz {
            3.0 * hz / 200.0
        } else {
            min_log_mel + (hz / min_log_hertz).ln() * logstep
        }
    };
    let mel_to_hz = |mel: f64| -> f64 {
        if mel < min_log_mel {
            200.0 * mel / 3.0
        } else {
            min_log_hertz * ((mel - min_log_mel) / logstep).exp()
        }
    };

    // n_mels + 2 equally spaced points in slaney mel space
    let mel_min = hz_to_mel(fmin64);
    let mel_max = hz_to_mel(fmax64);
    let n_points = n_mels + 2;
    let mel_points: Vec<f64> = (0..n_points)
        .map(|i| mel_min + (mel_max - mel_min) * (i as f64) / (n_points as f64 - 1.0))
        .collect();
    let filter_freqs: Vec<f64> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();

    // FFT bin center frequencies (linear in Hz) - matches np.linspace(0, sr//2, n_freqs)
    let fft_freqs: Vec<f64> = (0..n_freqs)
        .map(|i| (sr as f64 / 2.0) * (i as f64) / (n_freqs as f64 - 1.0))
        .collect();

    // Triangular filters using Python's _create_triangular_filter_bank algorithm:
    // For mel band m (0-indexed), filter_freqs indices are [m, m+1, m+2] = [left, center, right]
    // down_slope[k,m] = (fft_freqs[k] - filter_freqs[m]) / (filter_freqs[m+1] - filter_freqs[m])
    // up_slope[k,m]   = (filter_freqs[m+2] - fft_freqs[k]) / (filter_freqs[m+2] - filter_freqs[m+1])
    // filter[k,m] = max(0, min(down_slope, up_slope))
    let filter_diff: Vec<f64> = filter_freqs.windows(2).map(|w| w[1] - w[0]).collect();
    let mut filters = vec![vec![0.0_f64; n_freqs]; n_mels];

    for m in 0..n_mels {
        for k in 0..n_freqs {
            let down_slope = (fft_freqs[k] - filter_freqs[m]) / filter_diff[m];
            let up_slope = (filter_freqs[m + 2] - fft_freqs[k]) / filter_diff[m + 1];
            filters[m][k] = 0.0_f64.max(down_slope.min(up_slope));
        }
    }

    // Slaney normalization: scale each band by 2 / (f_right - f_left)
    for m in 0..n_mels {
        let bandwidth = filter_freqs[m + 2] - filter_freqs[m];
        if bandwidth > 0.0 {
            let enorm = 2.0 / bandwidth;
            for k in 0..n_freqs {
                filters[m][k] *= enorm;
            }
        }
    }

    filters
}

/// Apply reflect padding to audio, matching `np.pad(waveform, pad_width, mode='reflect')`.
///
/// Pads `pad` samples on each side. For reflect mode, sample at index -1 maps to index 1,
/// index -2 maps to index 2, etc.
fn reflect_pad(audio: &[f32], pad: usize) -> Vec<f32> {
    let n = audio.len();
    let mut out = Vec::with_capacity(n + 2 * pad);

    // Left padding (reflect)
    for i in (1..=pad).rev() {
        let idx = if i < n { i } else { n - 1 };
        out.push(audio[idx]);
    }

    // Original audio
    out.extend_from_slice(audio);

    // Right padding (reflect)
    for i in 1..=pad {
        let idx = if n >= i + 1 { n - 1 - i } else { 0 };
        out.push(audio[idx]);
    }

    out
}

/// PCM f32 → log-mel spectrogram (Whisper-compatible).
///
/// Processing (matches Python WhisperFeatureExtractor):
/// 1. Center-pad audio with n_fft/2 reflect padding on each side
/// 2. STFT (Hann window, n_fft=400, hop=160)
/// 3. Drop last frame (to get exactly n_frames_target frames)
/// 4. Apply slaney mel filterbank + floor 1e-10
/// 5. log10 transform
/// 6. Clamp to (max - 8.0)
/// 7. Normalize: (x + 4.0) / 4.0
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

    // Center-pad with reflect padding (Whisper convention)
    let pad = n_fft / 2;
    let padded = reflect_pad(audio, pad);

    // Build mel filterbank: shape (n_mels, n_bins)
    let filters = mel_filterbank(n_mels, n_fft, config.sample_rate, config.fmin, config.fmax);

    // Compute STFT power spectrum on padded audio
    let window = hann_window(n_fft);
    let power_frames = stft_power(&padded, n_fft, hop, &window);
    // Drop last frame to match Python's `log_spec[:, :-1]`
    let actual_frames = if power_frames.len() > 0 {
        power_frames.len() - 1
    } else {
        0
    };

    // Apply mel filterbank with floor in f64 (matching Python's float64 pipeline)
    let floor = 1e-10_f64;
    let mut mel = vec![0.0_f64; n_mels * n_frames_target];

    for m in 0..n_mels {
        for t in 0..n_frames_target {
            if t < actual_frames {
                let mut sum = 0.0_f64;
                for k in 0..n_bins {
                    sum += filters[m][k] * power_frames[t][k] as f64;
                }
                mel[m * n_frames_target + t] = sum.max(floor);
            } else {
                mel[m * n_frames_target + t] = floor;
            }
        }
    }

    // log10 transform (in f64)
    for val in mel.iter_mut() {
        *val = val.log10();
    }

    // Clamp to (max - 8.0)
    let log_max = mel.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let clamp_min = log_max - 8.0;
    for val in mel.iter_mut() {
        *val = (*val).max(clamp_min);
    }

    // Whisper normalization: (x + 4.0) / 4.0, then cast back to f32
    mel.iter().map(|&v| ((v + 4.0) / 4.0) as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slaney_mel_known_values() {
        // Below 1000 Hz: linear
        let mel_200 = hz_to_mel_slaney(200.0);
        assert!((mel_200 - 3.0).abs() < 0.01, "200 Hz => 3.0 mel, got {mel_200}");

        let mel_1000 = hz_to_mel_slaney(1000.0);
        assert!((mel_1000 - 15.0).abs() < 0.01, "1000 Hz => 15.0 mel, got {mel_1000}");

        // Above 1000 Hz: logarithmic
        let mel_8000 = hz_to_mel_slaney(8000.0);
        assert!((mel_8000 - 45.245).abs() < 0.1, "8000 Hz => ~45.2 mel, got {mel_8000}");
    }

    #[test]
    fn test_slaney_roundtrip() {
        for &hz in &[0.0_f32, 200.0, 700.0, 1000.0, 2000.0, 4000.0, 8000.0] {
            let roundtrip = mel_to_hz_slaney(hz_to_mel_slaney(hz));
            assert!(
                (roundtrip - hz).abs() < 0.5,
                "roundtrip failed for {hz}: got {roundtrip}"
            );
        }
    }

    #[test]
    fn test_filterbank_shape() {
        let fb = mel_filterbank(80, 400, 16000, 0.0, 8000.0);
        assert_eq!(fb.len(), 80);
        assert_eq!(fb[0].len(), 201);
    }

    #[test]
    fn test_filterbank_no_negative() {
        let fb = mel_filterbank(80, 400, 16000, 0.0, 8000.0);
        for (m, row) in fb.iter().enumerate() {
            for (k, &val) in row.iter().enumerate() {
                assert!(val >= 0.0, "negative value {val} at mel={m}, bin={k}");
            }
        }
    }

    #[test]
    fn test_log_mel_silence_value() {
        let config = MelConfig::default();
        let audio = vec![0.0_f32; 128000];
        let mel = log_mel_spectrogram(&audio, &config);

        assert_eq!(mel.len(), 64000);

        // Silence: all values should be -1.5 (= (-10 + 4) / 4)
        for (i, &v) in mel.iter().enumerate() {
            assert!(
                (v - (-1.5)).abs() < 0.01,
                "silence: mel[{}] = {}, expected -1.5",
                i, v
            );
        }
    }

    #[test]
    fn test_log_mel_output_shape() {
        let config = MelConfig::default();
        let audio = vec![0.0_f32; 128000];
        let mel = log_mel_spectrogram(&audio, &config);
        assert_eq!(mel.len(), 64000);
    }

    #[test]
    fn test_log_mel_non_nan() {
        let config = MelConfig::default();
        let audio: Vec<f32> = (0..128000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();
        let mel = log_mel_spectrogram(&audio, &config);

        for (i, &v) in mel.iter().enumerate() {
            assert!(!v.is_nan(), "NaN at index {}", i);
            assert!(!v.is_infinite(), "Inf at index {}", i);
        }
    }
}
