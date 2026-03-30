//! Pre-Whisper speech filter: RMS energy + spectral voice band analysis.
//!
//! Determines whether an audio segment likely contains human speech
//! before sending it to Whisper, preventing hallucination on noise.
//!
//! Two independent checks computed from one FFT pass:
//! 1. **RMS**: Is there enough signal energy?
//! 2. **Spectral voice ratio**: Is energy concentrated in human voice bands (85–4000 Hz)?

use rustfft::{num_complex::Complex, FftPlanner};

/// Sample rate of audio in the pipeline.
const SAMPLE_RATE: f32 = 16000.0;

/// Human voice frequency range.
const VOICE_FREQ_LOW: f32 = 85.0;
const VOICE_FREQ_HIGH: f32 = 4000.0;

/// Estimated speaker gender from fundamental frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceGender {
    Male,
    Female,
    Child,
    Unknown,
}

impl std::fmt::Display for VoiceGender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoiceGender::Male => write!(f, "male"),
            VoiceGender::Female => write!(f, "female"),
            VoiceGender::Child => write!(f, "child"),
            VoiceGender::Unknown => write!(f, "unknown"),
        }
    }
}

/// Result of speech likelihood analysis.
#[derive(Debug, Clone)]
pub struct SpeechFilterResult {
    /// RMS energy of the audio signal.
    pub rms: f32,
    /// Ratio of energy in voice band (85–4000 Hz) to total energy [0.0–1.0].
    pub voice_band_ratio: f32,
    /// Whether the audio passes the speech filter.
    pub is_speech: bool,
    /// Estimated fundamental frequency (F0) in Hz. 0.0 if not detected.
    pub f0_hz: f32,
    /// Estimated speaker gender based on F0.
    pub gender: VoiceGender,
}

/// Configuration for the speech filter.
#[derive(Debug, Clone)]
pub struct SpeechFilterConfig {
    /// Minimum RMS energy to consider as potential speech.
    pub rms_threshold: f32,
    /// Minimum voice band energy ratio to consider as speech.
    pub voice_band_ratio_threshold: f32,
    /// Minimum audio duration in milliseconds to send to Whisper.
    /// Filters out short impulse sounds (clicks, pops) that pass RMS/spectral checks.
    /// Set to 0 to disable.
    pub min_audio_ms: u64,
}

impl Default for SpeechFilterConfig {
    fn default() -> Self {
        Self {
            rms_threshold: 200.0,
            voice_band_ratio_threshold: 0.90,
            min_audio_ms: 800,
        }
    }
}

/// Analyze audio segment for speech likelihood.
///
/// Computes RMS and spectral voice band ratio in a single pass.
/// Returns both metrics and a combined is_speech decision.
pub fn analyze(audio_i16: &[i16], config: &SpeechFilterConfig) -> SpeechFilterResult {
    if audio_i16.is_empty() {
        return SpeechFilterResult {
            rms: 0.0,
            voice_band_ratio: 0.0,
            is_speech: false,
            f0_hz: 0.0,
            gender: VoiceGender::Unknown,
        };
    }

    // --- RMS (time domain, no FFT needed) ---
    let sum_sq: f64 = audio_i16.iter().map(|&s| (s as f64) * (s as f64)).sum();
    let rms = (sum_sq / audio_i16.len() as f64).sqrt() as f32;

    // Early exit: if RMS is too low, no need for FFT
    if rms < config.rms_threshold {
        return SpeechFilterResult {
            rms,
            voice_band_ratio: 0.0,
            is_speech: false,
            f0_hz: 0.0,
            gender: VoiceGender::Unknown,
        };
    }

    // --- Spectral analysis (voice band ratio + F0 + gender) ---
    let spectral = compute_spectral_features(audio_i16);

    let is_speech = spectral.voice_band_ratio >= config.voice_band_ratio_threshold;

    SpeechFilterResult {
        rms,
        voice_band_ratio: spectral.voice_band_ratio,
        is_speech,
        f0_hz: spectral.f0_hz,
        gender: spectral.gender,
    }
}

/// F0 detection frequency range.
const F0_MIN_HZ: f32 = 85.0;
const F0_MAX_HZ: f32 = 400.0;

/// Spectral analysis result from a single FFT pass.
struct SpectralResult {
    voice_band_ratio: f32,
    f0_hz: f32,
    gender: VoiceGender,
}

/// Compute voice band ratio + F0 detection from a single FFT pass.
fn compute_spectral_features(audio_i16: &[i16]) -> SpectralResult {
    let n_fft = audio_i16.len().next_power_of_two();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n_fft);

    // Convert i16 → f32 complex, zero-pad to n_fft
    let mut buffer: Vec<Complex<f32>> = Vec::with_capacity(n_fft);
    for &s in audio_i16 {
        buffer.push(Complex::new(s as f32, 0.0));
    }
    buffer.resize(n_fft, Complex::new(0.0, 0.0));

    fft.process(&mut buffer);

    // Compute power spectrum (only first half — positive frequencies)
    let n_bins = n_fft / 2 + 1;
    let freq_resolution = SAMPLE_RATE / n_fft as f32;

    let voice_bin_low = (VOICE_FREQ_LOW / freq_resolution).ceil() as usize;
    let voice_bin_high = (VOICE_FREQ_HIGH / freq_resolution).floor() as usize;
    let voice_bin_high = voice_bin_high.min(n_bins - 1);

    let f0_bin_low = (F0_MIN_HZ / freq_resolution).ceil() as usize;
    let f0_bin_high = (F0_MAX_HZ / freq_resolution).floor() as usize;
    let f0_bin_high = f0_bin_high.min(n_bins - 1);

    let mut total_energy: f64 = 0.0;
    let mut voice_energy: f64 = 0.0;
    let mut f0_peak_bin: usize = 0;
    let mut f0_peak_power: f64 = 0.0;

    for (i, c) in buffer[..n_bins].iter().enumerate() {
        let power = (c.re * c.re + c.im * c.im) as f64;
        total_energy += power;
        if i >= voice_bin_low && i <= voice_bin_high {
            voice_energy += power;
        }
        // Track peak in F0 range
        if i >= f0_bin_low && i <= f0_bin_high && power > f0_peak_power {
            f0_peak_power = power;
            f0_peak_bin = i;
        }
    }

    let voice_band_ratio = if total_energy < 1e-10 {
        0.0
    } else {
        (voice_energy / total_energy) as f32
    };

    // F0 detection: only valid if peak is significantly above noise floor
    let avg_power = total_energy / n_bins as f64;
    let f0_hz = if f0_peak_power > avg_power * 10.0 {
        f0_peak_bin as f32 * freq_resolution
    } else {
        0.0
    };

    let gender = classify_gender(f0_hz);

    SpectralResult {
        voice_band_ratio,
        f0_hz,
        gender,
    }
}

/// Classify speaker gender from fundamental frequency.
fn classify_gender(f0_hz: f32) -> VoiceGender {
    if f0_hz < 1.0 {
        VoiceGender::Unknown
    } else if f0_hz < 165.0 {
        VoiceGender::Male
    } else if f0_hz < 255.0 {
        VoiceGender::Female
    } else if f0_hz <= 400.0 {
        VoiceGender::Child
    } else {
        VoiceGender::Unknown
    }
}

/// Analyze audio using both serial and parallel execution, returning
/// the result along with timing information.
///
/// Serial: RMS first, skip FFT if RMS too low.
/// Parallel: RMS and FFT computed concurrently.
pub fn analyze_with_benchmark(
    audio_i16: &[i16],
    config: &SpeechFilterConfig,
) -> (SpeechFilterResult, std::time::Duration, std::time::Duration) {
    use std::time::Instant;

    // --- Serial ---
    let t0 = Instant::now();
    let result_serial = analyze(audio_i16, config);
    let serial_dur = t0.elapsed();

    // --- Parallel ---
    let t0 = Instant::now();
    let result_parallel = analyze_parallel(audio_i16, config);
    let parallel_dur = t0.elapsed();

    // Use serial result (they should be identical)
    let _ = result_parallel;
    (result_serial, serial_dur, parallel_dur)
}

/// Parallel version: RMS and FFT computed concurrently via threads.
fn analyze_parallel(audio_i16: &[i16], config: &SpeechFilterConfig) -> SpeechFilterResult {
    if audio_i16.is_empty() {
        return SpeechFilterResult {
            rms: 0.0,
            voice_band_ratio: 0.0,
            is_speech: false,
            f0_hz: 0.0,
            gender: VoiceGender::Unknown,
        };
    }

    let audio_for_fft = audio_i16.to_vec();
    let fft_handle = std::thread::spawn(move || compute_spectral_features(&audio_for_fft));

    // RMS on current thread
    let sum_sq: f64 = audio_i16.iter().map(|&s| (s as f64) * (s as f64)).sum();
    let rms = (sum_sq / audio_i16.len() as f64).sqrt() as f32;

    let spectral = fft_handle.join().unwrap_or(SpectralResult {
        voice_band_ratio: 0.0,
        f0_hz: 0.0,
        gender: VoiceGender::Unknown,
    });

    let is_speech =
        rms >= config.rms_threshold && spectral.voice_band_ratio >= config.voice_band_ratio_threshold;

    SpeechFilterResult {
        rms,
        voice_band_ratio: spectral.voice_band_ratio,
        is_speech,
        f0_hz: spectral.f0_hz,
        gender: spectral.gender,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_rejected() {
        let audio = vec![0i16; 16000];
        let result = analyze(&audio, &SpeechFilterConfig::default());
        assert!(!result.is_speech);
        assert!(result.rms < 1.0);
    }

    #[test]
    fn sine_440hz_in_voice_band() {
        // 440 Hz sine wave — should be in voice band, detected as child (250-400 not matching, it's actually > 400? No, 440 > 400 → Unknown)
        // Actually 440 Hz is outside F0 range (85-400), so gender should be Unknown
        let audio: Vec<i16> = (0..16000)
            .map(|i| (f32::sin(2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0) * 10000.0) as i16)
            .collect();
        let result = analyze(&audio, &SpeechFilterConfig::default());
        assert!(result.rms > 100.0);
        assert!(result.voice_band_ratio > 0.5);
        assert!(result.is_speech);
    }

    #[test]
    fn high_freq_noise_rejected() {
        // 7000 Hz sine — outside voice band
        let audio: Vec<i16> = (0..16000)
            .map(|i| (f32::sin(2.0 * std::f32::consts::PI * 7000.0 * i as f32 / 16000.0) * 10000.0) as i16)
            .collect();
        let result = analyze(&audio, &SpeechFilterConfig::default());
        assert!(result.rms > 100.0);
        assert!(result.voice_band_ratio < 0.3);
        assert!(!result.is_speech);
    }

    #[test]
    fn benchmark_serial_vs_parallel() {
        let audio: Vec<i16> = (0..16000)
            .map(|i| (f32::sin(2.0 * std::f32::consts::PI * 300.0 * i as f32 / 16000.0) * 5000.0) as i16)
            .collect();
        let config = SpeechFilterConfig::default();
        let (result, serial, parallel) = analyze_with_benchmark(&audio, &config);
        assert!(result.is_speech);
        assert!(serial.as_nanos() > 0);
        assert!(parallel.as_nanos() > 0);
    }

    #[test]
    fn f0_male_120hz() {
        let audio: Vec<i16> = (0..16000)
            .map(|i| (f32::sin(2.0 * std::f32::consts::PI * 120.0 * i as f32 / 16000.0) * 10000.0) as i16)
            .collect();
        let result = analyze(&audio, &SpeechFilterConfig::default());
        assert!((result.f0_hz - 120.0).abs() < 10.0, "expected ~120 Hz, got {}", result.f0_hz);
        assert_eq!(result.gender, VoiceGender::Male);
    }

    #[test]
    fn f0_female_210hz() {
        let audio: Vec<i16> = (0..16000)
            .map(|i| (f32::sin(2.0 * std::f32::consts::PI * 210.0 * i as f32 / 16000.0) * 10000.0) as i16)
            .collect();
        let result = analyze(&audio, &SpeechFilterConfig::default());
        assert!((result.f0_hz - 210.0).abs() < 10.0, "expected ~210 Hz, got {}", result.f0_hz);
        assert_eq!(result.gender, VoiceGender::Female);
    }

    #[test]
    fn f0_child_300hz() {
        let audio: Vec<i16> = (0..16000)
            .map(|i| (f32::sin(2.0 * std::f32::consts::PI * 300.0 * i as f32 / 16000.0) * 10000.0) as i16)
            .collect();
        let result = analyze(&audio, &SpeechFilterConfig::default());
        assert!((result.f0_hz - 300.0).abs() < 10.0, "expected ~300 Hz, got {}", result.f0_hz);
        assert_eq!(result.gender, VoiceGender::Child);
    }

    #[test]
    fn classify_gender_boundaries() {
        assert_eq!(classify_gender(0.0), VoiceGender::Unknown);
        assert_eq!(classify_gender(100.0), VoiceGender::Male);
        assert_eq!(classify_gender(164.9), VoiceGender::Male);
        assert_eq!(classify_gender(165.0), VoiceGender::Female);
        assert_eq!(classify_gender(254.9), VoiceGender::Female);
        assert_eq!(classify_gender(255.0), VoiceGender::Child);
        assert_eq!(classify_gender(400.0), VoiceGender::Child);
        assert_eq!(classify_gender(401.0), VoiceGender::Unknown);
    }
}
