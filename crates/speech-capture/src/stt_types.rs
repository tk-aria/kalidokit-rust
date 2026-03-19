/// STT operation mode.
#[derive(Debug, Clone, Default)]
pub enum SttMode {
    /// STT disabled — VoiceEnd returns audio only.
    #[default]
    Disabled,
    /// Batch — transcribe entire utterance at VoiceEnd.
    Batch,
    /// Streaming — emit interim results every `interim_interval_ms` during speech,
    /// then final result at VoiceEnd.
    Streaming { interim_interval_ms: u32 },
}

/// Whisper STT configuration.
#[derive(Debug, Clone)]
pub struct SttConfig {
    /// Path to Whisper model file (e.g. "models/ggml-base.bin").
    pub model_path: String,
    /// Language hint (None = auto-detect, Some("ja") = Japanese).
    pub language: Option<String>,
    /// Operation mode.
    pub mode: SttMode,
}
