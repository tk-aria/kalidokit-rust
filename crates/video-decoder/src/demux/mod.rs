//! Container demuxing: extract video packets from MP4/MOV files.

use std::time::Duration;

use crate::error::Result;
use crate::types::Codec;

mod mp4;
pub use mp4::Mp4Demuxer;

/// A demuxed video packet containing a NAL unit and its timing information.
pub struct VideoPacket {
    /// Raw NAL unit data (AVCC framing, not Annex-B).
    pub data: Vec<u8>,
    /// Presentation timestamp.
    pub pts: Duration,
    /// Decode timestamp.
    pub dts: Duration,
    /// Whether this packet is a sync (key) frame.
    pub is_keyframe: bool,
}

/// Codec-specific parameter sets extracted from the container.
pub struct CodecParameters {
    /// The video codec used by this track.
    pub codec: Codec,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Frames per second (computed from sample table).
    pub fps: f64,
    /// Total duration of the video track.
    pub duration: Duration,
    /// Codec-specific extra data (e.g., avcC record for H.264).
    pub extra_data: Vec<u8>,
}

/// Container demuxer trait.
///
/// Implementations read packets sequentially from a video container
/// and support seeking to keyframe-aligned positions.
pub trait Demuxer: Send {
    /// Returns the codec parameters for this video track.
    fn parameters(&self) -> &CodecParameters;

    /// Read the next video packet, or `None` at end-of-stream.
    fn next_packet(&mut self) -> Result<Option<VideoPacket>>;

    /// Seek to the nearest keyframe at or before `position`.
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
