//! GStreamer VA-API decoder backend (Linux fallback path).

#[cfg(all(target_os = "linux", feature = "gstreamer"))]
use std::time::Duration;

#[cfg(all(target_os = "linux", feature = "gstreamer"))]
use crate::error::{Result, VideoError};
#[cfg(all(target_os = "linux", feature = "gstreamer"))]
use crate::session::{OutputTarget, SessionConfig, VideoSession};
#[cfg(all(target_os = "linux", feature = "gstreamer"))]
use crate::types::*;

/// GStreamer VA-API decoder session (Linux fallback).
///
/// Uses `vaapidecodebin ! appsink` pipeline for VA-API HW decode,
/// then imports the DMA-BUF fd into VkImage or wgpu texture.
#[cfg(all(target_os = "linux", feature = "gstreamer"))]
pub struct GstVideoSession {
    _placeholder: (),
}

#[cfg(all(target_os = "linux", feature = "gstreamer"))]
impl GstVideoSession {
    pub fn new(_path: &str, _output: &OutputTarget, _config: &SessionConfig) -> Result<Self> {
        Err(VideoError::Decode(
            "GStreamer VA-API not yet implemented".into(),
        ))
    }
}

#[cfg(all(target_os = "linux", feature = "gstreamer"))]
impl VideoSession for GstVideoSession {
    fn info(&self) -> &VideoInfo {
        unreachable!()
    }
    fn position(&self) -> Duration {
        unreachable!()
    }
    fn decode_frame(&mut self, _dt: Duration) -> Result<FrameStatus> {
        unreachable!()
    }
    fn seek(&mut self, _position: Duration) -> Result<()> {
        unreachable!()
    }
    fn set_looping(&mut self, _looping: bool) {
        unreachable!()
    }
    fn is_looping(&self) -> bool {
        unreachable!()
    }
    fn pause(&mut self) {
        unreachable!()
    }
    fn resume(&mut self) {
        unreachable!()
    }
    fn is_paused(&self) -> bool {
        unreachable!()
    }
    fn backend(&self) -> Backend {
        Backend::GStreamerVaapi
    }
}
