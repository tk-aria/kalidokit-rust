use thiserror::Error;

#[derive(Debug, Error)]
pub enum VideoError {
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),

    #[error("no compatible HW decoder found")]
    NoHwDecoder,

    #[error("demux error: {0}")]
    Demux(String),

    #[error("decode error: {0}")]
    Decode(String),

    #[error("GPU interop error: {0}")]
    GpuInterop(String),

    #[error("seek error: {0}")]
    Seek(String),

    #[error("output target format mismatch: expected {expected}, got {actual}")]
    FormatMismatch { expected: String, actual: String },

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

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
