use std::time::Duration;

use crate::error::Result;
use crate::handle::NativeHandle;
use crate::types::{Backend, ColorSpace, FrameStatus, PixelFormat, VideoInfo};

/// Output texture information provided by the application.
#[derive(Debug, Clone, Copy)]
pub struct OutputTarget {
    pub native_handle: NativeHandle,
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub color_space: ColorSpace,
}

/// Session configuration.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub looping: bool,
    pub preferred_backend: Option<Backend>,
    pub allow_software_fallback: bool,
    pub decode_buffer_size: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            looping: true,
            preferred_backend: None,
            allow_software_fallback: true,
            decode_buffer_size: 4,
        }
    }
}

/// A video decode session bound to one video file and one output texture.
///
/// # Lifecycle
/// 1. Create via [`crate::open()`]
/// 2. Call [`decode_frame()`](VideoSession::decode_frame) every frame
/// 3. When [`FrameStatus::NewFrame`] is returned, the texture has been updated
/// 4. Drop to release all resources
pub trait VideoSession: Send {
    fn info(&self) -> &VideoInfo;
    fn position(&self) -> Duration;
    fn decode_frame(&mut self, dt: Duration) -> Result<FrameStatus>;
    fn seek(&mut self, position: Duration) -> Result<()>;
    fn set_looping(&mut self, looping: bool);
    fn is_looping(&self) -> bool;
    fn pause(&mut self);
    fn resume(&mut self);
    fn is_paused(&self) -> bool;
    fn backend(&self) -> Backend;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_config_default() {
        let cfg = SessionConfig::default();
        assert!(cfg.looping);
        assert!(cfg.preferred_backend.is_none());
        assert!(cfg.allow_software_fallback);
        assert_eq!(cfg.decode_buffer_size, 4);
    }

    #[test]
    fn output_target_construction() {
        let ot = OutputTarget {
            native_handle: NativeHandle::Wgpu {
                queue: std::ptr::null(),
                texture_id: 1,
            },
            format: PixelFormat::Rgba8Srgb,
            width: 1920,
            height: 1080,
            color_space: ColorSpace::default(),
        };
        assert_eq!(ot.width, 1920);
        assert_eq!(ot.height, 1080);
    }
}
