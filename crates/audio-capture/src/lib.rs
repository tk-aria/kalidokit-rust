//! Cross-platform microphone audio capture.
//!
//! Captures from the default input device, resamples to 16kHz mono i16.

pub mod resample;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// A frame of captured audio.
#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// 16kHz mono 16-bit PCM samples.
    pub samples: Vec<i16>,
    /// Always 16000.
    pub sample_rate: u32,
    /// Time since capture started.
    pub timestamp: Duration,
}

/// Configuration for audio capture.
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// Specific device name, or None for default.
    pub device_name: Option<String>,
    /// Samples per output frame (default: 256).
    pub frame_size: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            frame_size: 256,
        }
    }
}

/// Errors from audio capture.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no input device found")]
    DeviceNotFound,
    #[error("stream error: {0}")]
    StreamError(String),
    #[error("unsupported format: {0}")]
    FormatError(String),
}

/// Cross-platform microphone audio capture.
///
/// Captures from the system's input device, resamples to 16kHz mono i16,
/// and delivers frames via callback.
pub struct AudioCapture {
    config: AudioConfig,
    running: Arc<AtomicBool>,
    stream: Option<cpal::Stream>,
}

impl AudioCapture {
    pub fn new(config: AudioConfig) -> Result<Self, AudioError> {
        Ok(Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            stream: None,
        })
    }

    /// Start capturing. `callback` receives AudioFrame with 16kHz mono i16 data.
    pub fn start<F>(&mut self, mut callback: F) -> Result<(), AudioError>
    where
        F: FnMut(AudioFrame) + Send + 'static,
    {
        let host = cpal::default_host();
        let device = match &self.config.device_name {
            Some(name) => host
                .input_devices()
                .map_err(|e| AudioError::StreamError(e.to_string()))?
                .find(|d| {
                    d.description()
                        .ok()
                        .map(|desc| desc.name() == name.as_str())
                        .unwrap_or(false)
                })
                .ok_or(AudioError::DeviceNotFound)?,
            None => host
                .default_input_device()
                .ok_or(AudioError::DeviceNotFound)?,
        };

        let supported = device
            .default_input_config()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

        let src_rate = supported.sample_rate();
        let channels = supported.channels() as usize;
        let frame_size = self.config.frame_size;
        let running = self.running.clone();
        let start_time = Instant::now();

        // Accumulator for resampled mono samples
        let mut buffer: Vec<i16> = Vec::new();

        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &supported.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }
                    let mono = resample::downmix_to_mono(data, channels);
                    let resampled = resample::resample_nearest(&mono, src_rate, 16000);
                    let pcm = resample::f32_to_i16(&resampled);
                    buffer.extend_from_slice(&pcm);

                    while buffer.len() >= frame_size {
                        let frame_samples: Vec<i16> = buffer.drain(..frame_size).collect();
                        callback(AudioFrame {
                            samples: frame_samples,
                            sample_rate: 16000,
                            timestamp: start_time.elapsed(),
                        });
                    }
                },
                |err| log::error!("Audio stream error: {err}"),
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &supported.into(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }
                    let floats: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                    let mono = resample::downmix_to_mono(&floats, channels);
                    let resampled = resample::resample_nearest(&mono, src_rate, 16000);
                    let pcm = resample::f32_to_i16(&resampled);
                    buffer.extend_from_slice(&pcm);

                    while buffer.len() >= frame_size {
                        let frame_samples: Vec<i16> = buffer.drain(..frame_size).collect();
                        callback(AudioFrame {
                            samples: frame_samples,
                            sample_rate: 16000,
                            timestamp: start_time.elapsed(),
                        });
                    }
                },
                |err| log::error!("Audio stream error: {err}"),
                None,
            ),
            other => return Err(AudioError::FormatError(format!("{other:?}"))),
        }
        .map_err(|e| AudioError::StreamError(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;
        self.running.store(true, Ordering::Relaxed);
        self.stream = Some(stream);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        self.stream = None;
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// List available input device names.
    pub fn list_devices() -> Result<Vec<String>, AudioError> {
        let host = cpal::default_host();
        let devices = host
            .input_devices()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;
        Ok(devices
            .filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
            .collect())
    }
}
