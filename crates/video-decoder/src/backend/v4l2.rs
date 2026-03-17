//! V4L2 M2M decoder backend (Linux SBC / embedded path).

#[cfg(all(target_os = "linux", feature = "v4l2"))]
use std::time::Duration;

#[cfg(all(target_os = "linux", feature = "v4l2"))]
use crate::error::{Result, VideoError};
#[cfg(all(target_os = "linux", feature = "v4l2"))]
use crate::session::{OutputTarget, SessionConfig, VideoSession};
#[cfg(all(target_os = "linux", feature = "v4l2"))]
use crate::types::*;

/// V4L2 M2M decoder session (Linux SBC / embedded).
///
/// Uses `/dev/video*` V4L2 memory-to-memory codec device for HW decode
/// on Raspberry Pi, Rockchip, etc. Output buffers are DMA-BUF exported.
#[cfg(all(target_os = "linux", feature = "v4l2"))]
pub struct V4l2VideoSession {
    _placeholder: (),
}

#[cfg(all(target_os = "linux", feature = "v4l2"))]
impl V4l2VideoSession {
    pub fn new(_path: &str, _output: &OutputTarget, _config: &SessionConfig) -> Result<Self> {
        Err(VideoError::Decode("V4L2 M2M not yet implemented".into()))
    }

    /// Check if a V4L2 M2M decode device is available.
    pub fn is_supported() -> bool {
        false
    }
}

#[cfg(all(target_os = "linux", feature = "v4l2"))]
impl VideoSession for V4l2VideoSession {
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
        Backend::V4l2
    }
}
