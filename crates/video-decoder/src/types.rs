use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    H265,
    Vp9,
    Av1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8Srgb,
    Rgba8Unorm,
    Bgra8Srgb,
    Bgra8Unorm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSpace {
    Bt601,
    #[default]
    Bt709,
    Srgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStatus {
    NewFrame,
    Waiting,
    EndOfStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    VideoToolbox,
    D3d12Video,
    MediaFoundation,
    VulkanVideo,
    GStreamerVaapi,
    V4l2,
    MediaCodec,
    Software,
}

#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub duration: Duration,
    pub fps: f64,
    pub backend: Backend,
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
