//! Android MediaCodec decoder backend.

#[cfg(target_os = "android")]
use std::time::Duration;

#[cfg(target_os = "android")]
use crate::error::{Result, VideoError};
#[cfg(target_os = "android")]
use crate::session::{OutputTarget, SessionConfig, VideoSession};
#[cfg(target_os = "android")]
use crate::types::*;

/// Android MediaCodec decoder session.
///
/// Uses AMediaCodec NDK API for HW-accelerated decode with
/// AHardwareBuffer output for zero-copy Vulkan interop.
#[cfg(target_os = "android")]
pub struct McVideoSession {
    _placeholder: (),
}

#[cfg(target_os = "android")]
impl McVideoSession {
    pub fn new(_path: &str, _output: &OutputTarget, _config: &SessionConfig) -> Result<Self> {
        Err(VideoError::Decode("MediaCodec not yet implemented".into()))
    }
}

#[cfg(target_os = "android")]
impl VideoSession for McVideoSession {
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
        Backend::MediaCodec
    }
}
