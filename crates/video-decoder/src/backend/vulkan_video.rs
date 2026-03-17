//! Vulkan Video decoder backend (Linux primary path).

#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
use crate::error::{Result, VideoError};
#[cfg(target_os = "linux")]
use crate::session::{OutputTarget, SessionConfig, VideoSession};
#[cfg(target_os = "linux")]
use crate::types::*;

/// Vulkan Video decoder session (Linux primary path).
///
/// Uses VK_KHR_video_decode_queue + VK_KHR_video_decode_h264/h265
/// for Vulkan-native HW decode with zero-copy to VkImage output.
#[cfg(target_os = "linux")]
pub struct VkVideoSession {
    _placeholder: (),
}

#[cfg(target_os = "linux")]
impl VkVideoSession {
    pub fn new(_path: &str, _output: &OutputTarget, _config: &SessionConfig) -> Result<Self> {
        Err(VideoError::Decode(
            "Vulkan Video not yet implemented".into(),
        ))
    }

    /// Runtime detection: check VK_KHR_video_decode_queue support.
    pub fn is_supported(
        _instance: *mut std::ffi::c_void,
        _physical_device: *mut std::ffi::c_void,
    ) -> bool {
        false
    }
}

#[cfg(target_os = "linux")]
impl VideoSession for VkVideoSession {
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
        Backend::VulkanVideo
    }
}
