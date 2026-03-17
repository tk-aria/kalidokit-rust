use std::time::Duration;

/// Video codec identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    /// H.264 / AVC.
    H264,
    /// H.265 / HEVC.
    H265,
    /// VP9.
    Vp9,
    /// AV1.
    Av1,
}

/// Pixel format of the output texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// RGBA 8-bit sRGB.
    Rgba8Srgb,
    /// RGBA 8-bit linear (unorm).
    Rgba8Unorm,
    /// BGRA 8-bit sRGB.
    Bgra8Srgb,
    /// BGRA 8-bit linear (unorm).
    Bgra8Unorm,
}

/// YUV color space used by the source video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSpace {
    /// ITU-R BT.601 (SD video).
    Bt601,
    /// ITU-R BT.709 (HD video). This is the default.
    #[default]
    Bt709,
    /// sRGB (computer graphics).
    Srgb,
}

/// Result of a single `decode_frame()` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStatus {
    /// A new frame has been decoded and written to the output texture.
    NewFrame,
    /// No new frame is available yet (e.g., not enough time elapsed).
    Waiting,
    /// The video has reached the end and looping is disabled.
    EndOfStream,
}

/// Platform-specific video decoder backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// macOS / iOS: VideoToolbox via AVFoundation.
    VideoToolbox,
    /// Windows: D3D12 Video Decode API.
    D3d12Video,
    /// Windows: Media Foundation (fallback).
    MediaFoundation,
    /// Linux: Vulkan Video Extensions.
    VulkanVideo,
    /// Linux: GStreamer VA-API pipeline (optional feature).
    GStreamerVaapi,
    /// Linux: V4L2 Stateless codec (optional feature, e.g., Raspberry Pi).
    V4l2,
    /// Android: MediaCodec + AHardwareBuffer.
    MediaCodec,
    /// CPU fallback: openh264 software decoder.
    Software,
}

/// Metadata about an open video stream.
#[derive(Debug, Clone)]
pub struct VideoInfo {
    /// The video codec.
    pub codec: Codec,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Total duration of the video.
    pub duration: Duration,
    /// Frames per second.
    pub fps: f64,
    /// The decoder backend in use.
    pub backend: Backend,
    /// Whether the backend produces NV12 that needs GPU color conversion.
    pub needs_color_conversion: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_space_default_is_bt709() {
        assert_eq!(ColorSpace::default(), ColorSpace::Bt709);
    }

    #[test]
    fn codec_clone_and_eq() {
        let a = Codec::H264;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn backend_debug_format() {
        let b = Backend::Software;
        assert_eq!(format!("{:?}", b), "Software");
    }

    #[test]
    fn frame_status_variants() {
        assert_ne!(FrameStatus::NewFrame, FrameStatus::Waiting);
        assert_ne!(FrameStatus::Waiting, FrameStatus::EndOfStream);
    }

    #[test]
    fn video_info_clone() {
        let info = VideoInfo {
            codec: Codec::H264,
            width: 1920,
            height: 1080,
            duration: Duration::from_secs(10),
            fps: 30.0,
            backend: Backend::Software,
            needs_color_conversion: false,
        };
        let cloned = info.clone();
        assert_eq!(cloned.width, 1920);
        assert_eq!(cloned.codec, Codec::H264);
    }
}
