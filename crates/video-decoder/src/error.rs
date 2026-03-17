use thiserror::Error;

/// Errors that can occur during video decoding operations.
#[derive(Debug, Error)]
pub enum VideoError {
    /// The video uses a codec not supported by the current backend.
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),

    /// No hardware decoder is available for the current platform/GPU.
    #[error("no compatible HW decoder found")]
    NoHwDecoder,

    /// Container demuxing failed (e.g., corrupt MP4, missing tracks).
    #[error("demux error: {0}")]
    Demux(String),

    /// Frame decoding failed (e.g., bitstream error, decoder crash).
    #[error("decode error: {0}")]
    Decode(String),

    /// GPU interop failed (e.g., texture import, DMA-BUF mapping).
    #[error("GPU interop error: {0}")]
    GpuInterop(String),

    /// Seek operation failed (e.g., no keyframes, invalid position).
    #[error("seek error: {0}")]
    Seek(String),

    /// The output texture format does not match what the decoder produces.
    #[error("output target format mismatch: expected {expected}, got {actual}")]
    FormatMismatch {
        /// The format the decoder expected.
        expected: String,
        /// The format that was provided.
        actual: String,
    },

    /// The specified video file does not exist.
    #[error("file not found: {0}")]
    FileNotFound(String),

    /// An opaque error from an underlying library.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A type alias for `std::result::Result<T, VideoError>`.
pub type Result<T> = std::result::Result<T, VideoError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_unsupported_codec() {
        let e = VideoError::UnsupportedCodec("vp9".into());
        assert_eq!(e.to_string(), "unsupported codec: vp9");
    }

    #[test]
    fn display_no_hw_decoder() {
        let e = VideoError::NoHwDecoder;
        assert_eq!(e.to_string(), "no compatible HW decoder found");
    }

    #[test]
    fn display_format_mismatch() {
        let e = VideoError::FormatMismatch {
            expected: "RGBA8".into(),
            actual: "NV12".into(),
        };
        assert!(e.to_string().contains("RGBA8"));
        assert!(e.to_string().contains("NV12"));
    }

    #[test]
    fn from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("test error");
        let video_err: VideoError = anyhow_err.into();
        assert!(matches!(video_err, VideoError::Other(_)));
        assert!(video_err.to_string().contains("test error"));
    }
}
