//! Cross-platform audio capture (input and output loopback).
//!
//! Captures from input (microphone) or output (loopback) devices,
//! resamples to 16kHz mono i16.

pub mod resample;
#[cfg(target_os = "macos")]
pub mod sc_audio;

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
    /// Which source this frame came from.
    pub source: AudioSource,
}

/// Audio source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSource {
    /// Microphone / line-in (input device).
    Input,
    /// System audio loopback (output device).
    ///
    /// On macOS, requires a loopback virtual device (e.g. BlackHole, Soundflower)
    /// or ScreenCaptureKit. The device name should be specified in `AudioConfig`.
    Output,
}

/// Configuration for audio capture.
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// Specific device name, or None for default.
    pub device_name: Option<String>,
    /// Samples per output frame (default: 256).
    pub frame_size: usize,
    /// Audio source type (default: Input).
    pub source: AudioSource,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            frame_size: 256,
            source: AudioSource::Input,
        }
    }
}

/// Errors from audio capture.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no {0} device found")]
    DeviceNotFound(&'static str),
    #[error("stream error: {0}")]
    StreamError(String),
    #[error("unsupported format: {0}")]
    FormatError(String),
}

/// Active stream backend.
enum StreamBackend {
    Cpal(cpal::Stream),
    #[cfg(target_os = "macos")]
    ScreenCaptureKit(sc_audio::ScAudioCapture),
}

/// Cross-platform audio capture.
///
/// Captures from input (microphone) or output (loopback) devices,
/// resamples to 16kHz mono i16, and delivers frames via callback.
///
/// Output capture strategy by platform:
/// - **macOS 14.2+**: CATapDescription via cpal (automatic)
/// - **macOS 12.3–14.1**: ScreenCaptureKit (automatic fallback)
/// - **Windows**: WASAPI loopback via cpal (automatic)
/// - **Linux**: PulseAudio/PipeWire monitor source (specify device_name)
pub struct AudioCapture {
    config: AudioConfig,
    running: Arc<AtomicBool>,
    backend: Option<StreamBackend>,
}

impl AudioCapture {
    pub fn new(config: AudioConfig) -> Result<Self, AudioError> {
        Ok(Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            backend: None,
        })
    }

    /// Resolve the cpal device and its default config based on AudioSource.
    fn resolve_device(
        config: &AudioConfig,
    ) -> Result<(cpal::Device, cpal::SupportedStreamConfig), AudioError> {
        let host = cpal::default_host();
        match config.source {
            AudioSource::Input => {
                let device = match &config.device_name {
                    Some(name) => host
                        .input_devices()
                        .map_err(|e| AudioError::StreamError(e.to_string()))?
                        .find(|d| {
                            d.description()
                                .ok()
                                .map(|desc| desc.name() == name.as_str())
                                .unwrap_or(false)
                        })
                        .ok_or(AudioError::DeviceNotFound("input"))?,
                    None => host
                        .default_input_device()
                        .ok_or(AudioError::DeviceNotFound("input"))?,
                };
                let supported = device
                    .default_input_config()
                    .map_err(|e| AudioError::StreamError(e.to_string()))?;
                Ok((device, supported))
            }
            AudioSource::Output => {
                // --- Strategy ---
                //
                // macOS 14.2+: cpal can tap output devices directly via
                //   CATapDescription (ScreenCaptureKit). We resolve the output
                //   device and use its input config (cpal creates the tap).
                //
                // macOS < 14.2 / other: A loopback virtual audio device
                //   (e.g. BlackHole) that appears as an input device is required.
                //   The user must specify the device_name.
                //
                // Windows (WASAPI): cpal supports loopback capture on output
                //   devices natively.
                //
                // Linux (PulseAudio/PipeWire): Monitor sources appear as input
                //   devices. Specify the monitor device name.

                if let Some(name) = &config.device_name {
                    // Explicit device name: search input devices first (loopback
                    // drivers register there), then output devices.
                    let from_input = host
                        .input_devices()
                        .map_err(|e| AudioError::StreamError(e.to_string()))?
                        .find(|d| {
                            d.description()
                                .ok()
                                .map(|desc| desc.name() == name.as_str())
                                .unwrap_or(false)
                        });
                    let device = match from_input {
                        Some(d) => d,
                        None => host
                            .output_devices()
                            .map_err(|e| AudioError::StreamError(e.to_string()))?
                            .find(|d| {
                                d.description()
                                    .ok()
                                    .map(|desc| desc.name() == name.as_str())
                                    .unwrap_or(false)
                            })
                            .ok_or(AudioError::DeviceNotFound("output/loopback"))?,
                    };
                    let supported = device
                        .default_input_config()
                        .or_else(|_| device.default_output_config())
                        .map_err(|e| AudioError::StreamError(e.to_string()))?;
                    return Ok((device, supported));
                }

                // No device name: try the default output device directly.
                // This works on macOS 14.2+ (CATapDescription) and Windows (WASAPI loopback).
                let device = host
                    .default_output_device()
                    .ok_or(AudioError::DeviceNotFound("output"))?;

                // Check if the output device supports input config (tap/loopback).
                match device.default_input_config() {
                    Ok(supported) => Ok((device, supported)),
                    Err(_) => {
                        // Output device doesn't support input capture.
                        // Provide a helpful error with platform-specific guidance.
                        let mut msg = String::from(
                            "Cannot capture from the default output device directly. ",
                        );
                        #[cfg(target_os = "macos")]
                        {
                            msg.push_str(
                                "On macOS < 14.2, install a loopback driver (e.g. BlackHole) \
                                 and specify it via AudioConfig::device_name. \
                                 On macOS 14.2+, output capture should work automatically.",
                            );
                        }
                        #[cfg(target_os = "linux")]
                        {
                            msg.push_str(
                                "On Linux, specify a PulseAudio/PipeWire monitor source name \
                                 via AudioConfig::device_name.",
                            );
                        }
                        #[cfg(target_os = "windows")]
                        {
                            msg.push_str(
                                "On Windows, WASAPI loopback should work. \
                                 Try specifying the device name explicitly.",
                            );
                        }
                        Err(AudioError::StreamError(msg))
                    }
                }
            }
        }
    }

    /// Start capturing. `callback` receives AudioFrame with 16kHz mono i16 data.
    ///
    /// For output capture on macOS < 14.2, automatically falls back to
    /// ScreenCaptureKit (requires screen recording permission).
    pub fn start<F>(&mut self, callback: F) -> Result<(), AudioError>
    where
        F: FnMut(AudioFrame) + Send + 'static,
    {
        // On macOS < 14.2 with Output source and no device name,
        // skip cpal entirely — calling default_input_config() on a pure output
        // device can segfault on older macOS. Go straight to ScreenCaptureKit.
        #[cfg(target_os = "macos")]
        {
            if self.config.source == AudioSource::Output
                && self.config.device_name.is_none()
                && sc_audio::is_available()
                && !sc_audio::has_catap()
            {
                log::info!(
                    "macOS < 14.2 detected, using ScreenCaptureKit for output capture"
                );
                return self.start_screencapturekit(callback);
            }
        }

        // Try cpal.
        match Self::resolve_device(&self.config) {
            Ok((device, supported)) => {
                return self.start_cpal(&device, supported, callback);
            }
            Err(cpal_err) => {
                // cpal failed for Output source → try ScreenCaptureKit on macOS.
                if self.config.source == AudioSource::Output {
                    #[cfg(target_os = "macos")]
                    {
                        if sc_audio::is_available() {
                            log::info!(
                                "cpal output capture unavailable, falling back to ScreenCaptureKit"
                            );
                            return self.start_screencapturekit(callback);
                        }
                    }
                }
                return Err(cpal_err);
            }
        }
    }

    /// Start capture via cpal backend.
    fn start_cpal<F>(
        &mut self,
        device: &cpal::Device,
        supported: cpal::SupportedStreamConfig,
        callback: F,
    ) -> Result<(), AudioError>
    where
        F: FnMut(AudioFrame) + Send + 'static,
    {
        let src_rate = supported.sample_rate();
        let channels = supported.channels() as usize;
        let frame_size = self.config.frame_size;
        let source = self.config.source;
        let running = self.running.clone();
        let start_time = Instant::now();

        let stream = Self::build_input_stream(
            device,
            supported,
            running,
            start_time,
            src_rate,
            channels,
            frame_size,
            source,
            callback,
        )?;

        stream
            .play()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;
        self.running.store(true, Ordering::Relaxed);
        self.backend = Some(StreamBackend::Cpal(stream));
        Ok(())
    }

    /// Start capture via ScreenCaptureKit (macOS 12.3+ fallback).
    #[cfg(target_os = "macos")]
    fn start_screencapturekit<F>(&mut self, callback: F) -> Result<(), AudioError>
    where
        F: FnMut(AudioFrame) + Send + 'static,
    {
        let mut sc = sc_audio::ScAudioCapture::new(self.config.frame_size);
        sc.start(callback)?;
        self.running.store(true, Ordering::Relaxed);
        self.backend = Some(StreamBackend::ScreenCaptureKit(sc));
        Ok(())
    }

    /// Build a cpal input stream with the given format, handling F32/I16 sample types.
    fn build_input_stream<F>(
        device: &cpal::Device,
        supported: cpal::SupportedStreamConfig,
        running: Arc<AtomicBool>,
        start_time: Instant,
        src_rate: u32,
        channels: usize,
        frame_size: usize,
        source: AudioSource,
        mut callback: F,
    ) -> Result<cpal::Stream, AudioError>
    where
        F: FnMut(AudioFrame) + Send + 'static,
    {
        let mut buffer: Vec<i16> = Vec::new();

        let emit = move |buffer: &mut Vec<i16>,
                         callback: &mut F,
                         data_f32: &[f32],
                         start_time: Instant| {
            let mono = resample::downmix_to_mono(data_f32, channels);
            let resampled = resample::resample_nearest(&mono, src_rate, 16000);
            let pcm = resample::f32_to_i16(&resampled);
            buffer.extend_from_slice(&pcm);

            while buffer.len() >= frame_size {
                let frame_samples: Vec<i16> = buffer.drain(..frame_size).collect();
                callback(AudioFrame {
                    samples: frame_samples,
                    sample_rate: 16000,
                    timestamp: start_time.elapsed(),
                    source,
                });
            }
        };

        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &supported.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }
                    emit(&mut buffer, &mut callback, data, start_time);
                },
                |err| log::error!("Audio stream error: {err}"),
                None,
            ),
            cpal::SampleFormat::I16 => {
                let running2 = running;
                device.build_input_stream(
                    &supported.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if !running2.load(Ordering::Relaxed) {
                            return;
                        }
                        let floats: Vec<f32> =
                            data.iter().map(|&s| s as f32 / 32768.0).collect();
                        emit(&mut buffer, &mut callback, &floats, start_time);
                    },
                    |err| log::error!("Audio stream error: {err}"),
                    None,
                )
            }
            other => return Err(AudioError::FormatError(format!("{other:?}"))),
        }
        .map_err(|e| AudioError::StreamError(e.to_string()))?;

        Ok(stream)
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        #[cfg(target_os = "macos")]
        if let Some(StreamBackend::ScreenCaptureKit(ref mut sc)) = self.backend {
            sc.stop();
        }
        self.backend = None;
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// List available input device names.
    pub fn list_input_devices() -> Result<Vec<String>, AudioError> {
        let host = cpal::default_host();
        let devices = host
            .input_devices()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;
        Ok(devices
            .filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
            .collect())
    }

    /// List available output device names.
    pub fn list_output_devices() -> Result<Vec<String>, AudioError> {
        let host = cpal::default_host();
        let devices = host
            .output_devices()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;
        Ok(devices
            .filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
            .collect())
    }

    /// List all device names (input + output).
    pub fn list_devices() -> Result<Vec<String>, AudioError> {
        let mut all = Self::list_input_devices()?;
        let output = Self::list_output_devices()?;
        for name in output {
            if !all.contains(&name) {
                all.push(name);
            }
        }
        Ok(all)
    }
}
