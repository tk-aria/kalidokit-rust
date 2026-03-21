// Short-Time Fourier Transform for ETD pipeline.
// Step 1.4: Hann window and STFT power spectrum computation using rustfft.

use rustfft::{num_complex::Complex, FftPlanner};

/// Compute a Hann window of the given size.
///
/// w(n) = 0.5 * (1 - cos(2 * pi * n / (N - 1)))
///
/// Returns a `Vec<f32>` of length `size`. If `size <= 1`, returns a vec of ones.
pub fn hann_window(size: usize) -> Vec<f32> {
    if size <= 1 {
        return vec![1.0; size];
    }
    (0..size)
        .map(|n| {
            0.5 * (1.0
                - (2.0 * std::f32::consts::PI * n as f32 / (size as f32 - 1.0)).cos())
        })
        .collect()
}

/// Compute the STFT power spectrum (magnitude squared) of an audio signal.
///
/// Applies the given `window` to each frame, computes the FFT via `rustfft`,
/// and returns the squared magnitudes of the first `n_fft / 2 + 1` bins.
///
/// # Returns
///
/// `Vec<Vec<f32>>` of shape `(n_frames, n_fft / 2 + 1)` where
/// `n_frames = (audio.len() - n_fft) / hop + 1` (only complete frames).
///
/// Returns an empty `Vec` when `audio` is empty or shorter than `n_fft`.
pub fn stft_power(audio: &[f32], n_fft: usize, hop: usize, window: &[f32]) -> Vec<Vec<f32>> {
    if audio.len() < n_fft || n_fft == 0 || hop == 0 {
        return Vec::new();
    }

    let n_bins = n_fft / 2 + 1;
    let n_frames = (audio.len() - n_fft) / hop + 1;

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n_fft);

    let mut frames = Vec::with_capacity(n_frames);
    let mut buffer = vec![Complex::new(0.0_f32, 0.0); n_fft];

    for f in 0..n_frames {
        let start = f * hop;

        // Fill buffer with windowed samples.
        for i in 0..n_fft {
            let sample = audio[start + i];
            let w = if i < window.len() { window[i] } else { 1.0 };
            buffer[i] = Complex::new(sample * w, 0.0);
        }

        fft.process(&mut buffer);

        // Collect power (magnitude squared) for the first n_bins.
        let power: Vec<f32> = buffer[..n_bins]
            .iter()
            .map(|c| c.re * c.re + c.im * c.im)
            .collect();

        frames.push(power);
    }

    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hann_window_endpoints() {
        let n = 512;
        let w = hann_window(n);
        assert_eq!(w.len(), n);
        assert!(w[0].abs() < 1e-6, "hann[0] should be ~0, got {}", w[0]);
        assert!(
            (w[n / 2] - 1.0).abs() < 1e-3,
            "hann[N/2] should be ~1, got {}",
            w[n / 2]
        );
    }

    #[test]
    fn test_hann_window_symmetry() {
        let n = 400;
        let w = hann_window(n);
        for i in 0..n / 2 {
            let diff = (w[i] - w[n - 1 - i]).abs();
            assert!(
                diff < 1e-6,
                "hann[{}] ({}) != hann[{}] ({})",
                i,
                w[i],
                n - 1 - i,
                w[n - 1 - i]
            );
        }
    }

    #[test]
    fn test_stft_silence() {
        let n_fft = 400;
        let hop = 160;
        let window = hann_window(n_fft);
        let audio = vec![0.0_f32; 16000]; // 1 second of silence at 16kHz

        let power = stft_power(&audio, n_fft, hop, &window);
        assert!(!power.is_empty());
        for (f, frame) in power.iter().enumerate() {
            for (b, &val) in frame.iter().enumerate() {
                assert!(
                    val.abs() < 1e-10,
                    "silence power[{}][{}] = {}, expected ~0",
                    f,
                    b,
                    val
                );
            }
        }
    }

    #[test]
    fn test_stft_sine_wave() {
        let sr = 16000.0_f32;
        let freq = 440.0_f32;
        let n_fft = 400;
        let hop = 160;
        let duration_samples = 16000; // 1 second
        let window = hann_window(n_fft);

        // Generate 440 Hz sine wave.
        let audio: Vec<f32> = (0..duration_samples)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin())
            .collect();

        let power = stft_power(&audio, n_fft, hop, &window);
        assert!(!power.is_empty());

        // Expected bin for 440 Hz: bin = freq * n_fft / sr = 440 * 400 / 16000 = 11.
        let expected_bin = (freq * n_fft as f32 / sr).round() as usize;

        // Check a frame in the middle (avoid edge effects).
        let mid_frame = power.len() / 2;
        let frame = &power[mid_frame];

        // Find the bin with maximum power.
        let (peak_bin, _peak_val) = frame
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        assert!(
            (peak_bin as isize - expected_bin as isize).unsigned_abs() <= 1,
            "peak at bin {}, expected near bin {}",
            peak_bin,
            expected_bin
        );
    }

    #[test]
    fn test_stft_output_shape() {
        let n_fft = 400;
        let hop = 160;
        let n_samples = 128000; // 8 seconds at 16kHz
        let window = hann_window(n_fft);
        let audio = vec![0.0_f32; n_samples];

        let power = stft_power(&audio, n_fft, hop, &window);

        let expected_frames = (n_samples - n_fft) / hop + 1; // (128000-400)/160+1 = 799
        let expected_bins = n_fft / 2 + 1; // 201

        assert_eq!(
            power.len(),
            expected_frames,
            "expected {} frames, got {}",
            expected_frames,
            power.len()
        );
        assert_eq!(
            power[0].len(),
            expected_bins,
            "expected {} bins, got {}",
            expected_bins,
            power[0].len()
        );
    }

    #[test]
    fn test_stft_empty_audio() {
        let window = hann_window(400);
        let power = stft_power(&[], 400, 160, &window);
        assert!(power.is_empty(), "empty audio should produce empty output");
    }

    #[test]
    fn test_stft_shorter_than_nfft() {
        let n_fft = 400;
        let window = hann_window(n_fft);
        let audio = vec![1.0_f32; 200]; // shorter than n_fft

        let power = stft_power(&audio, n_fft, 160, &window);
        assert!(
            power.is_empty(),
            "audio shorter than n_fft should produce 0 frames, got {}",
            power.len()
        );
    }
}
