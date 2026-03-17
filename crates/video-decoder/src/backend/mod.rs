use crate::error::{Result, VideoError};
use crate::session::{OutputTarget, SessionConfig, VideoSession};

/// Create a video session with automatic backend selection.
pub fn create_session(
    path: &str,
    _output: OutputTarget,
    _config: SessionConfig,
) -> Result<Box<dyn VideoSession>> {
    if !std::path::Path::new(path).exists() {
        return Err(VideoError::FileNotFound(path.to_string()));
    }
    Err(VideoError::NoHwDecoder)
}
