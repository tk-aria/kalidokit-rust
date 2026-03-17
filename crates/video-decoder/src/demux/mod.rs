use std::time::Duration;

use crate::error::Result;
use crate::types::Codec;

mod mp4;
pub use mp4::Mp4Demuxer;

/// A demuxed video packet (NAL unit with timing).
pub struct VideoPacket {
    pub data: Vec<u8>,
    pub pts: Duration,
    pub dts: Duration,
    pub is_keyframe: bool,
}

/// Codec-specific parameter sets extracted from the container.
pub struct CodecParameters {
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: Duration,
    pub extra_data: Vec<u8>,
}

/// Container demuxer trait.
pub trait Demuxer: Send {
    fn parameters(&self) -> &CodecParameters;
    fn next_packet(&mut self) -> Result<Option<VideoPacket>>;
    fn seek(&mut self, position: Duration) -> Result<()>;
}

/// Create a demuxer based on file extension.
pub fn create_demuxer(path: &str) -> crate::error::Result<Box<dyn Demuxer>> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mp4" | "m4v" | "mov" => Ok(Box::new(Mp4Demuxer::new(path)?)),
        _ => Err(crate::error::VideoError::UnsupportedCodec(format!(
            "unsupported container: .{}",
            ext
        ))),
    }
}
