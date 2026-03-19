//! Voice activity detection from microphone input.
//!
//! Combines `audio-capture` and `ten-vad` (vad) to detect speech segments
//! from microphone input in real time.

mod segmenter;
pub mod stt_types;

#[cfg(feature = "stt")]
mod whisper_engine;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

pub use audio_capture::{AudioConfig, AudioError};
pub use stt_types::{SttConfig, SttMode};

/// Events emitted during speech capture.
#[derive(Debug, Clone)]
pub enum SpeechEvent {
    /// Voice activity started.
    VoiceStart { timestamp: Duration },
    /// Interim transcription result (streaming mode only).
    TranscriptInterim { timestamp: Duration, text: String },
    /// Voice activity ended with the captured audio.
    VoiceEnd {
        timestamp: Duration,
        audio: Vec<i16>,
        duration: Duration,
        /// Transcription text (only when STT is enabled).
        transcript: Option<String>,
    },
    /// Per-frame VAD status (only if `emit_vad_status` is enabled).
    VadStatus {
        timestamp: Duration,
        probability: f32,
        is_voice: bool,
    },
}

/// Configuration for speech capture.
#[derive(Debug, Clone)]
pub struct SpeechConfig {
    pub vad_threshold: f32,
    pub hop_size: vad::HopSize,
    /// Ignore utterances shorter than this.
    pub min_speech_duration_ms: u32,
    /// End speech after this much silence.
    pub silence_timeout_ms: u32,
    /// Emit VadStatus events per frame.
    pub emit_vad_status: bool,
    pub audio: AudioConfig,
    /// STT configuration. None = disabled, Some = enable Whisper.
    /// Requires the `stt` feature to actually run transcription.
    pub stt: Option<SttConfig>,
}

impl Default for SpeechConfig {
    fn default() -> Self {
        Self {
            vad_threshold: 0.5,
            hop_size: vad::HopSize::Samples256,
            min_speech_duration_ms: 200,
            silence_timeout_ms: 500,
            emit_vad_status: false,
            audio: AudioConfig::default(),
            stt: None,
        }
    }
}

/// Errors from speech capture.
#[derive(Debug, thiserror::Error)]
pub enum SpeechError {
    #[error("audio error: {0}")]
    Audio(#[from] AudioError),
    #[error("vad error: {0}")]
    Vad(#[from] vad::VadError),
    #[error("stt error: {0}")]
    Stt(String),
}

/// Real-time speech capture: microphone -> VAD -> speech events.
pub struct SpeechCapture {
    config: SpeechConfig,
    capture: audio_capture::AudioCapture,
    running: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl SpeechCapture {
    pub fn new(config: SpeechConfig) -> Result<Self, SpeechError> {
        let capture = audio_capture::AudioCapture::new(config.audio.clone())?;
        Ok(Self {
            config,
            capture,
            running: Arc::new(AtomicBool::new(false)),
            worker: None,
        })
    }

    /// Start speech capture. `callback` receives SpeechEvent.
    pub fn start<F>(&mut self, callback: F) -> Result<(), SpeechError>
    where
        F: FnMut(SpeechEvent) + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<audio_capture::AudioFrame>();

        // Start audio capture -> send frames to channel
        self.capture.start(move |frame| {
            let _ = tx.send(frame);
        })?;

        let running = self.running.clone();
        running.store(true, Ordering::Relaxed);

        let config = self.config.clone();
        let worker = std::thread::spawn(move || {
            Self::vad_worker(rx, callback, &config, &running);
        });

        self.worker = Some(worker);
        Ok(())
    }

    fn vad_worker<F>(
        rx: mpsc::Receiver<audio_capture::AudioFrame>,
        mut callback: F,
        config: &SpeechConfig,
        running: &AtomicBool,
    ) where
        F: FnMut(SpeechEvent),
    {
        let mut vad_instance = match vad::TenVad::new(config.hop_size, config.vad_threshold) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to create VAD: {e}");
                return;
            }
        };

        let mut seg =
            segmenter::VadSegmenter::new(config.min_speech_duration_ms, config.silence_timeout_ms);

        // Initialize Whisper engine if STT is configured and feature is enabled.
        #[cfg(feature = "stt")]
        let whisper_engine: Option<whisper_engine::WhisperEngine> =
            config.stt.as_ref().and_then(|stt_config| {
                match whisper_engine::WhisperEngine::new(stt_config) {
                    Ok(engine) => {
                        log::info!("Whisper STT engine initialized");
                        Some(engine)
                    }
                    Err(e) => {
                        log::error!("Failed to initialize Whisper STT: {e}");
                        None
                    }
                }
            });

        #[cfg(feature = "stt")]
        let stt_mode: SttMode = config
            .stt
            .as_ref()
            .map(|c| c.mode.clone())
            .unwrap_or(SttMode::Disabled);

        #[cfg(feature = "stt")]
        let mut last_interim_time = Duration::ZERO;

        while running.load(Ordering::Relaxed) {
            let frame = match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(f) => f,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };

            match vad_instance.process(&frame.samples) {
                Ok(result) => {
                    if config.emit_vad_status {
                        callback(SpeechEvent::VadStatus {
                            timestamp: frame.timestamp,
                            probability: result.probability,
                            is_voice: result.is_voice,
                        });
                    }

                    // Streaming interim transcription: emit partial results periodically.
                    #[cfg(feature = "stt")]
                    if let (
                        Some(engine),
                        SttMode::Streaming {
                            interim_interval_ms,
                        },
                    ) = (&whisper_engine, &stt_mode)
                    {
                        if seg.is_speaking() {
                            let elapsed = frame.timestamp.saturating_sub(last_interim_time);
                            if elapsed >= Duration::from_millis(*interim_interval_ms as u64) {
                                let accumulated = seg.accumulated_audio();
                                if !accumulated.is_empty() {
                                    if let Ok(text) = engine.transcribe(accumulated) {
                                        if !text.is_empty() {
                                            callback(SpeechEvent::TranscriptInterim {
                                                timestamp: frame.timestamp,
                                                text,
                                            });
                                        }
                                    }
                                }
                                last_interim_time = frame.timestamp;
                            }
                        }
                    }

                    for event in seg.feed(result.is_voice, &frame.samples, frame.timestamp) {
                        // Attach transcription to VoiceEnd when STT is enabled.
                        #[cfg(feature = "stt")]
                        if let SpeechEvent::VoiceEnd {
                            timestamp,
                            audio,
                            duration,
                            ..
                        } = &event
                        {
                            if let Some(engine) = &whisper_engine {
                                match &stt_mode {
                                    SttMode::Batch | SttMode::Streaming { .. } => {
                                        let transcript = engine.transcribe(audio).ok();
                                        callback(SpeechEvent::VoiceEnd {
                                            timestamp: *timestamp,
                                            audio: audio.clone(),
                                            duration: *duration,
                                            transcript,
                                        });
                                        // Reset interim timer for next utterance.
                                        #[cfg(feature = "stt")]
                                        {
                                            last_interim_time = Duration::ZERO;
                                        }
                                        continue;
                                    }
                                    SttMode::Disabled => {}
                                }
                            }
                        }

                        callback(event);
                    }
                }
                Err(e) => log::warn!("VAD process error: {e}"),
            }
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        self.capture.stop();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Drop for SpeechCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
