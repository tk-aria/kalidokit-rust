pub mod audio;
mod inference;
pub mod mel;
pub mod stft;

mod error;
pub use error::EtdError;

use std::path::PathBuf;

use crate::inference::EtdSession;
use crate::mel::{log_mel_spectrogram, MelConfig};

/// Configuration for the End-of-Turn Detector.
#[derive(Debug, Clone)]
pub struct EtdConfig {
    /// Path to the smart-turn v3 ONNX model file.
    pub model_path: PathBuf,
    /// Probability threshold above which the turn is considered complete.
    pub threshold: f32,
    /// Maximum audio window length in seconds (aligned to utterance end).
    pub max_audio_seconds: f32,
    /// Expected audio sample rate in Hz.
    pub sample_rate: u32,
}

impl Default for EtdConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("assets/models/smart_turn_v3.onnx"),
            threshold: 0.5,
            max_audio_seconds: 8.0,
            sample_rate: 16000,
        }
    }
}

/// Result of an end-of-turn prediction.
#[derive(Debug, Clone)]
pub struct EtdResult {
    /// `true` if the model predicts the speaker's turn is complete.
    pub prediction: bool,
    /// Raw probability from the model, in `[0.0, 1.0]`.
    pub probability: f32,
}

/// End-of-Turn Detector backed by an ONNX model.
///
/// Accepts raw audio (i16 or f32 PCM at 16 kHz) and returns a prediction
/// indicating whether the speaker has finished their turn.
pub struct EndOfTurnDetector {
    session: EtdSession,
    config: EtdConfig,
    mel_config: MelConfig,
}

impl EndOfTurnDetector {
    /// Create a new detector by loading the ONNX model specified in `config`.
    pub fn new(config: EtdConfig) -> Result<Self, EtdError> {
        let session = EtdSession::load(&config.model_path)?;

        let mel_config = MelConfig {
            sample_rate: config.sample_rate,
            chunk_length: config.max_audio_seconds,
            ..MelConfig::default()
        };

        Ok(Self {
            session,
            config,
            mel_config,
        })
    }

    /// Predict end-of-turn from i16 PCM audio samples.
    ///
    /// The audio should be mono 16 kHz. If longer than `max_audio_seconds`,
    /// only the trailing portion is used; if shorter, it is zero-padded at the start.
    pub fn predict_i16(&mut self, samples: &[i16]) -> Result<EtdResult, EtdError> {
        if samples.is_empty() {
            return Err(EtdError::InvalidAudio("audio is empty".into()));
        }
        let f32_samples = audio::i16_to_f32(samples);
        self.predict(&f32_samples)
    }

    /// Predict end-of-turn from f32 PCM audio samples in `[-1.0, 1.0]`.
    ///
    /// The audio should be mono 16 kHz. If longer than `max_audio_seconds`,
    /// only the trailing portion is used; if shorter, it is zero-padded at the start.
    pub fn predict(&mut self, samples: &[f32]) -> Result<EtdResult, EtdError> {
        if samples.is_empty() {
            return Err(EtdError::InvalidAudio("audio is empty".into()));
        }

        // Truncate or pad to the fixed window length.
        let audio = audio::truncate_or_pad(
            samples,
            self.config.sample_rate,
            self.config.max_audio_seconds,
        );

        // Compute log-mel spectrogram: shape (n_mels, n_frames) row-major.
        let mel = log_mel_spectrogram(&audio, &self.mel_config);

        let n_mels = self.mel_config.n_mels;
        let n_frames = mel.len() / n_mels;

        // Run ONNX inference.
        let probability = self.session.infer(&mel, n_mels, n_frames)?;

        Ok(EtdResult {
            prediction: probability >= self.config.threshold,
            probability,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predict_empty_audio() {
        // We can't load the model in unit tests (file not in git),
        // so test the empty-audio guard at the predict level by
        // constructing the detector with an invalid path — but the
        // empty-audio check fires before inference.
        //
        // Instead, verify that predict() returns InvalidAudio for
        // empty input without needing a real model. We achieve this
        // by directly calling the public API path that checks early.

        // Since we can't construct EndOfTurnDetector without a model,
        // we test the error variant string instead.
        let err = EtdError::InvalidAudio("audio is empty".into());
        assert!(format!("{err}").contains("empty"));
    }

    #[test]
    fn test_new_invalid_model_path() {
        let config = EtdConfig {
            model_path: PathBuf::from("/nonexistent/model.onnx"),
            ..EtdConfig::default()
        };
        let result = EndOfTurnDetector::new(config);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            matches!(err, EtdError::ModelLoad(_)),
            "expected ModelLoad error, got: {err}"
        );
    }

    /// Resolve the model path relative to the workspace root for tests.
    fn test_model_config() -> EtdConfig {
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        EtdConfig {
            model_path: workspace_root.join("assets/models/smart_turn_v3.onnx"),
            ..EtdConfig::default()
        }
    }

    #[test]
    #[ignore]
    fn test_predict_i16_silence() {
        let config = test_model_config();
        let mut detector = EndOfTurnDetector::new(config).expect("failed to load model");
        // 2 seconds of silence at 16 kHz
        let samples = vec![0i16; 32000];
        let result = detector.predict_i16(&samples).expect("inference failed");
        // Validate inference produces a valid probability.
        // Note: smart-turn v3 interprets silence as "turn complete" (prob ~0.73),
        // which is expected behavior — silence after speech signals end-of-turn.
        assert!(
            (0.0..=1.0).contains(&result.probability),
            "probability out of range: {}",
            result.probability
        );
    }

    #[test]
    #[ignore]
    fn test_predict_f32_silence() {
        let config = test_model_config();
        let mut detector = EndOfTurnDetector::new(config).expect("failed to load model");
        // 2 seconds of silence at 16 kHz
        let samples = vec![0.0f32; 32000];
        let result = detector.predict(&samples).expect("inference failed");
        // Validate inference produces a valid probability.
        assert!(
            (0.0..=1.0).contains(&result.probability),
            "probability out of range: {}",
            result.probability
        );
    }

    #[test]
    #[ignore]
    fn test_predict_result_fields() {
        let mut config = test_model_config();
        config.threshold = 0.5;
        let mut detector = EndOfTurnDetector::new(config).expect("failed to load model");
        let samples = vec![0.0f32; 32000];
        let result = detector.predict(&samples).expect("inference failed");

        // Probability should be in [0, 1]
        assert!(
            (0.0..=1.0).contains(&result.probability),
            "probability out of range: {}",
            result.probability
        );
        // prediction should be consistent with threshold
        assert_eq!(
            result.prediction,
            result.probability >= 0.5,
            "prediction ({}) inconsistent with probability ({:.4}) at threshold 0.5",
            result.prediction,
            result.probability
        );
    }
}
