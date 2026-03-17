//! # video-decoder
//!
//! Platform-native HW video decoder with zero-copy GPU texture output.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use video_decoder::{open, OutputTarget, SessionConfig, NativeHandle, PixelFormat, ColorSpace};
//!
//! // 1. Create a wgpu texture and get its native handle
//! // 2. Open a video session
//! // let session = open("video.mp4", output, SessionConfig::default())?;
//! // 3. Call session.decode_frame(dt) every frame
//! ```

pub mod backend;
pub mod convert;
pub mod demux;
pub mod error;
pub mod handle;
pub mod nal;
pub mod session;
pub mod types;
pub mod util;

pub use error::{Result, VideoError};
pub use handle::NativeHandle;
pub use session::{OutputTarget, SessionConfig, VideoSession};
pub use types::*;

/// Open a video file and create a decode session with automatic backend selection.
pub fn open(
    path: &str,
    output: OutputTarget,
    config: SessionConfig,
) -> Result<Box<dyn VideoSession>> {
    backend::create_session(path, output, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_output() -> OutputTarget {
        OutputTarget {
            native_handle: NativeHandle::Wgpu {
                queue: std::ptr::null(),
                texture_id: 0,
            },
            format: PixelFormat::Rgba8Srgb,
            width: 640,
            height: 480,
            color_space: ColorSpace::default(),
        }
    }

    #[test]
    fn open_nonexistent_file_returns_file_not_found() {
        let result = open(
            "/nonexistent/path.mp4",
            dummy_output(),
            SessionConfig::default(),
        );
        match result {
            Err(VideoError::FileNotFound(p)) => assert!(p.contains("nonexistent")),
            Err(other) => panic!("expected FileNotFound, got {:?}", other),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn open_invalid_mp4_returns_demux_error() {
        let dir = std::env::temp_dir().join("video_decoder_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dummy.mp4");
        std::fs::write(&path, b"not a real mp4").unwrap();

        let result = open(
            path.to_str().unwrap(),
            dummy_output(),
            SessionConfig::default(),
        );
        // With a Wgpu handle the software backend is attempted, which tries to
        // demux the file and fails because the content is not a valid MP4.
        match result {
            Err(VideoError::Demux(_)) => {}
            Err(other) => panic!("expected Demux error, got {:?}", other),
            Ok(_) => panic!("expected error"),
        }

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn open_with_no_software_fallback_returns_no_hw_decoder() {
        let dir = std::env::temp_dir().join("video_decoder_test_nosw");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dummy.mp4");
        std::fs::write(&path, b"not a real mp4").unwrap();

        let config = SessionConfig {
            allow_software_fallback: false,
            ..SessionConfig::default()
        };
        let result = open(path.to_str().unwrap(), dummy_output(), config);
        // Wgpu handle candidates = [Software], but software fallback is
        // disabled after candidates exhaust, so we get NoHwDecoder.
        match result {
            Err(VideoError::NoHwDecoder) => {}
            // Software is in the candidate list for Wgpu, so it will be tried
            // and fail with a Demux error before reaching the fallback check.
            Err(VideoError::Demux(_)) => {}
            Err(other) => panic!("expected NoHwDecoder or Demux, got {:?}", other),
            Ok(_) => panic!("expected error"),
        }

        std::fs::remove_file(&path).ok();
    }
}
