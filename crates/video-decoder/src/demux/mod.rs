use std::time::Duration;

use crate::error::Result;
use crate::types::Codec;

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
