//! H.264 NAL unit context using `h264-reader`.
//!
//! Parses SPS/PPS from raw avcC extra_data to extract resolution and codec parameters.

use std::convert::TryFrom;

use h264_reader::avcc::AvcDecoderConfigurationRecord;
use h264_reader::nal::pps::ParamSetId;

use crate::error::{Result, VideoError};

/// Holds parsed H.264 parameter sets and derived video properties.
pub struct H264Context {
    /// Raw SPS NAL unit bytes (without the length prefix).
    pub sps_bytes: Vec<Vec<u8>>,
    /// Raw PPS NAL unit bytes (without the length prefix).
    pub pps_bytes: Vec<Vec<u8>>,
    /// Decoded width in pixels.
    pub width: u32,
    /// Decoded height in pixels.
    pub height: u32,
    /// NAL length size in bytes (1..4), from the avcC record.
    pub nal_length_size: u8,
    /// The h264-reader context with parsed SPS/PPS for downstream use.
    pub context: h264_reader::Context,
}

impl H264Context {
    /// Create an H264Context from raw avcC (AVCDecoderConfigurationRecord) bytes.
    ///
    /// These bytes are typically found in the MP4 sample description box
    /// (`extra_data` from `CodecParameters`).
    pub fn from_avcc(avcc_data: &[u8]) -> Result<Self> {
        let avcc = AvcDecoderConfigurationRecord::try_from(avcc_data)
            .map_err(|e| VideoError::Demux(format!("invalid avcC record: {:?}", e)))?;

        let nal_length_size = avcc.length_size_minus_one() + 1;

        // Collect raw SPS bytes.
        let sps_bytes: Vec<Vec<u8>> = avcc
            .sequence_parameter_sets()
            .filter_map(|r| r.ok())
            .map(|b| b.to_vec())
            .collect();

        // Collect raw PPS bytes.
        let pps_bytes: Vec<Vec<u8>> = avcc
            .picture_parameter_sets()
            .filter_map(|r| r.ok())
            .map(|b| b.to_vec())
            .collect();

        // Build the h264-reader context (parses SPS/PPS internally).
        let context = avcc
            .create_context()
            .map_err(|e| VideoError::Demux(format!("failed to create H.264 context: {:?}", e)))?;

        // Extract dimensions from the first SPS.
        let (width, height) = context
            .sps_by_id(
                ParamSetId::from_u32(0).map_err(|_| VideoError::Demux("invalid SPS id".into()))?,
            )
            .and_then(|sps| sps.pixel_dimensions().ok())
            .or_else(|| {
                // Try all SPS entries if id 0 is not present.
                context.sps().find_map(|sps| sps.pixel_dimensions().ok())
            })
            .ok_or_else(|| VideoError::Demux("could not determine dimensions from SPS".into()))?;

        Ok(Self {
            sps_bytes,
            pps_bytes,
            width,
            height,
            nal_length_size,
            context,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known-good avcC from h264-reader's own test suite:
    // Baseline profile, 640x480
    #[test]
    fn parse_avcc_record() {
        let avcc_data: Vec<u8> = vec![
            0x01, // version
            0x42, // profile_idc = 66 (Baseline)
            0xc0, // constraint flags
            0x1e, // level_idc = 30
            0xff, // length_size_minus_one = 3 (NAL length = 4 bytes)
            0xe1, // num_sps = 1
            // SPS length = 32
            0x00, 0x20, //
            0x67, 0x42, 0xc0, 0x1e, 0xb9, 0x10, 0x61, 0xff, 0x78, 0x08, 0x80, 0x00, 0x00, 0x03,
            0x00, 0x80, 0x00, 0x00, 0x19, 0x71, 0x30, 0x06, 0xd6, 0x00, 0xda, 0xf7, 0xbd, 0xc0,
            0x7c, 0x22, 0x11, 0xa8, //
            // num_pps = 1
            0x01, //
            // PPS length = 4
            0x00, 0x04, //
            0x68, 0xde, 0x3c, 0x80,
        ];

        let ctx = H264Context::from_avcc(&avcc_data).unwrap();
        assert_eq!(ctx.nal_length_size, 4);
        assert_eq!(ctx.sps_bytes.len(), 1);
        assert_eq!(ctx.pps_bytes.len(), 1);
        // Verify dimensions were successfully extracted from the SPS.
        assert!(ctx.width > 0);
        assert!(ctx.height > 0);
    }

    #[test]
    fn empty_avcc_returns_error() {
        let result = H264Context::from_avcc(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn h264_context_from_fixture() {
        use crate::demux::{Demuxer, Mp4Demuxer};

        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/big_buck_bunny_360p.mp4");
        if !path.exists() {
            return;
        }
        let demuxer = Mp4Demuxer::new(path.to_str().unwrap()).unwrap();
        let extra = &demuxer.parameters().extra_data;
        assert!(!extra.is_empty(), "avcC extra_data should not be empty");
        let ctx = H264Context::from_avcc(extra).expect("should parse avcC");
        assert_eq!(ctx.width, 640);
        assert_eq!(ctx.height, 360);
    }
}
