//! Voice activity detection from microphone input.
//!
//! Combines `audio-capture` and `ten-vad` (vad) to detect speech segments
//! from microphone input in real time.

mod segmenter;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

pub use audio_capture::{AudioConfig, AudioError};

/// Events emitted during speech capture.
#[derive(Debug, Clone)]
pub enum SpeechEvent {
    /// Voice activity started.
    VoiceStart { timestamp: Duration },
    /// Voice activity ended with the captured audio.
    VoiceEnd {
        timestamp: Duration,
        audio: Vec<i16>,
        duration: Duration,
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

                    for event in seg.feed(result.is_voice, &frame.samples, frame.timestamp) {
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
