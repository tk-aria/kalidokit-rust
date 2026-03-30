#![cfg(feature = "stt")]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{SpeechError, SttConfig};

/// Result of a Whisper transcription including confidence metadata.
pub struct TranscribeResult {
    pub text: String,
    /// Maximum no_speech_probability across all segments.
    /// Higher = more likely the audio contains no speech (hallucination).
    pub no_speech_prob: f32,
}

pub struct WhisperEngine {
    ctx: whisper_rs::WhisperContext,
    language: Option<String>,
    heartbeat_ms: Arc<AtomicU64>,
    abort_flag: Arc<AtomicBool>,
}

impl WhisperEngine {
    pub fn new(config: &SttConfig) -> Result<Self, SpeechError> {
        let ctx = whisper_rs::WhisperContext::new_with_params(
            &config.model_path,
            whisper_rs::WhisperContextParameters::default(),
        )
        .map_err(|e| SpeechError::Stt(format!("Failed to load Whisper model: {e}")))?;

        Ok(Self {
            ctx,
            language: config.language.clone(),
            heartbeat_ms: Arc::new(AtomicU64::new(0)),
            abort_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Transcribe audio (i16 16kHz mono) and return full text.
    pub fn transcribe(&self, audio_i16: &[i16]) -> Result<String, SpeechError> {
        let result = self.transcribe_with_prob(audio_i16)?;
        Ok(result.text)
    }

    /// Transcribe audio and return text + no_speech_probability.
    pub fn transcribe_with_prob(&self, audio_i16: &[i16]) -> Result<TranscribeResult, SpeechError> {
        let audio_f32: Vec<f32> = audio_i16.iter().map(|&s| s as f32 / 32768.0).collect();

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| SpeechError::Stt(format!("Failed to create state: {e}")))?;

        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });

        if let Some(lang) = &self.language {
            params.set_language(Some(lang));
        }
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_single_segment(false);

        // Reset abort state and record initial heartbeat
        self.abort_flag.store(false, Ordering::Relaxed);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.heartbeat_ms.store(now_ms, Ordering::Relaxed);

        // Set abort callback that updates heartbeat and checks abort flag
        {
            let hb = Arc::clone(&self.heartbeat_ms);
            let af = Arc::clone(&self.abort_flag);
            params.set_abort_callback_safe(move || {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                hb.store(now_ms, Ordering::Relaxed);
                af.load(Ordering::Relaxed)
            });
        }

        let full_result = state.full(params, &audio_f32);

        // If aborted, return empty result instead of propagating the error
        if self.abort_flag.load(Ordering::Relaxed) {
            log::debug!("[STT-debug] Whisper inference was aborted");
            return Ok(TranscribeResult {
                text: String::new(),
                no_speech_prob: 0.0,
            });
        }

        full_result
            .map_err(|e| SpeechError::Stt(format!("Whisper transcription failed: {e}")))?;

        let mut text = String::new();
        let mut max_no_speech_prob: f32 = 0.0;
        let n_segments = state.full_n_segments();
        log::debug!(
            "[STT-debug] segments={}, audio_samples={}",
            n_segments,
            audio_i16.len()
        );
        for i in 0..n_segments {
            if let Some(segment) = state.get_segment(i) {
                let prob = segment.no_speech_probability();
                log::debug!(
                    "[STT-debug] seg[{}] no_speech_prob={:.6} text={:?}",
                    i,
                    prob,
                    segment.to_str().unwrap_or("(err)")
                );
                if prob > max_no_speech_prob {
                    max_no_speech_prob = prob;
                }
                if let Ok(segment_text) = segment.to_str() {
                    text.push_str(segment_text);
                }
            }
        }

        Ok(TranscribeResult {
            text: text.trim().to_string(),
            no_speech_prob: max_no_speech_prob,
        })
    }

    /// Get time since last Whisper progress update.
    pub fn heartbeat_age(&self) -> std::time::Duration {
        let last = self.heartbeat_ms.load(Ordering::Relaxed);
        if last == 0 {
            return std::time::Duration::ZERO;
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        std::time::Duration::from_millis(now_ms.saturating_sub(last))
    }

    /// Signal Whisper to abort current inference.
    pub fn abort(&self) {
        self.abort_flag.store(true, Ordering::Relaxed);
    }

    /// Check if abort has been signaled.
    pub fn is_aborting(&self) -> bool {
        self.abort_flag.load(Ordering::Relaxed)
    }
}
