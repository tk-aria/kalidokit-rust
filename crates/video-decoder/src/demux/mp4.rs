//! MP4/MOV container demuxer using `mp4parse`.
//!
//! mp4parse parses the moov box into a `MediaContext` with `Track` structs.
//! Each Track contains the sample table boxes (stco, stsz, stsc, stss, stts, ctts)
//! which we use to build a sample index and read individual NAL units from the file.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::time::Duration;

use crate::demux::{CodecParameters, Demuxer, VideoPacket};
use crate::error::{Result, VideoError};
use crate::types::Codec;

/// A resolved sample entry: file offset, size, timing, and sync flag.
#[derive(Debug, Clone)]
struct SampleEntry {
    offset: u64,
    size: u32,
    /// Decode timestamp in timescale units.
    dts_ticks: u64,
    /// Composition time offset in timescale units (signed).
    cts_offset: i64,
    is_sync: bool,
}

/// MP4/MOV container demuxer backed by `mp4parse`.
pub struct Mp4Demuxer {
    reader: BufReader<File>,
    params: CodecParameters,
    samples: Vec<SampleEntry>,
    /// Timescale (ticks per second) from the media header (mdhd).
    timescale: u64,
    /// Current sample index for sequential reading.
    cursor: usize,
    /// NAL length size (1..4 bytes) from the avcC box.
    /// Used by downstream decoders to split AVCC-framed NAL units.
    #[allow(dead_code)]
    nal_length_size: u8,
}

impl Mp4Demuxer {
    pub fn new(path: &str) -> Result<Self> {
        let file = File::open(path).map_err(|_| VideoError::FileNotFound(path.to_string()))?;
        let mut reader = BufReader::new(file);

        // Read the entire file into memory for mp4parse (it requires Read, not Seek).
        let mut buf = Vec::new();
        reader
            .read_to_end(&mut buf)
            .map_err(|e| VideoError::Demux(format!("failed to read file: {}", e)))?;

        let ctx = mp4parse::read_mp4(&mut std::io::Cursor::new(&buf))
            .map_err(|e| VideoError::Demux(format!("mp4parse error: {:?}", e)))?;

        // Find the first video track.
        let track = ctx
            .tracks
            .iter()
            .find(|t| t.track_type == mp4parse::TrackType::Video)
            .ok_or_else(|| VideoError::Demux("no video track found".into()))?;

        // Extract codec info.
        let stsd = track
            .stsd
            .as_ref()
            .ok_or_else(|| VideoError::Demux("missing stsd box".into()))?;

        let (codec, width, height, extra_data, nal_length_size) = extract_video_info(stsd)?;

        // Timescale from track.
        let timescale = track
            .timescale
            .map(|ts| ts.0)
            .ok_or_else(|| VideoError::Demux("missing track timescale".into()))?;

        // Compute track duration.
        let duration_ticks = track.duration.map(|d| d.0).unwrap_or(0);
        let duration = if timescale > 0 {
            Duration::from_secs_f64(duration_ticks as f64 / timescale as f64)
        } else {
            Duration::ZERO
        };

        // Build sample table.
        let samples = build_sample_table(track, timescale)?;

        // Compute FPS from total sample count and duration.
        let fps = if duration.as_secs_f64() > 0.0 {
            samples.len() as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        let params = CodecParameters {
            codec,
            width,
            height,
            fps,
            duration,
            extra_data,
        };

        // Reset reader to beginning for sample reading.
        let file = File::open(path)
            .map_err(|e| VideoError::Demux(format!("failed to reopen file: {}", e)))?;
        let reader = BufReader::new(file);

        Ok(Self {
            reader,
            params,
            samples,
            timescale,
            cursor: 0,
            nal_length_size,
        })
    }

    fn ticks_to_duration(&self, ticks: u64) -> Duration {
        if self.timescale == 0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(ticks as f64 / self.timescale as f64)
    }
}

impl Demuxer for Mp4Demuxer {
    fn parameters(&self) -> &CodecParameters {
        &self.params
    }

    fn next_packet(&mut self) -> Result<Option<VideoPacket>> {
        if self.cursor >= self.samples.len() {
            return Ok(None);
        }

        let entry = self.samples[self.cursor].clone();
        self.cursor += 1;

        // Seek and read sample data.
        self.reader
            .seek(SeekFrom::Start(entry.offset))
            .map_err(|e| VideoError::Demux(format!("seek failed: {}", e)))?;

        let mut raw = vec![0u8; entry.size as usize];
        self.reader
            .read_exact(&mut raw)
            .map_err(|e| VideoError::Demux(format!("read failed: {}", e)))?;

        // Convert AVCC (length-prefixed) → Annex B (start-code prefixed) NAL units.
        // For keyframes, prepend SPS/PPS from the avcC extra_data so the decoder
        // can initialize properly.
        let mut annex_b = Vec::with_capacity(raw.len() + 64);

        if entry.is_sync && !self.params.extra_data.is_empty() {
            // Prepend SPS and PPS NAL units from avcC.
            annex_b_from_avcc_extra(&self.params.extra_data, &mut annex_b);
        }

        avcc_to_annex_b(&raw, self.nal_length_size, &mut annex_b);

        let dts = self.ticks_to_duration(entry.dts_ticks);
        let pts_ticks = (entry.dts_ticks as i64 + entry.cts_offset).max(0) as u64;
        let pts = self.ticks_to_duration(pts_ticks);

        Ok(Some(VideoPacket {
            data: annex_b,
            pts,
            dts,
            is_keyframe: entry.is_sync,
        }))
    }

    fn seek(&mut self, position: Duration) -> Result<()> {
        if self.timescale == 0 {
            return Err(VideoError::Seek("zero timescale".into()));
        }

        let target_ticks = (position.as_secs_f64() * self.timescale as f64) as u64;

        // Find the latest sync sample at or before target_ticks.
        let mut best = 0usize;
        for (i, sample) in self.samples.iter().enumerate() {
            if sample.is_sync && sample.dts_ticks <= target_ticks {
                best = i;
            }
            if sample.dts_ticks > target_ticks {
                break;
            }
        }

        self.cursor = best;
        Ok(())
    }
}

// --- Helper functions ---

/// Extract video codec info from the sample description box.
fn extract_video_info(
    stsd: &mp4parse::SampleDescriptionBox,
) -> Result<(Codec, u32, u32, Vec<u8>, u8)> {
    for desc in stsd.descriptions.iter() {
        if let mp4parse::SampleEntry::Video(ref ve) = desc {
            let codec = match ve.codec_type {
                mp4parse::CodecType::H264 => Codec::H264,
                mp4parse::CodecType::AV1 => Codec::Av1,
                mp4parse::CodecType::VP9 => Codec::Vp9,
                other => {
                    return Err(VideoError::UnsupportedCodec(format!("{:?}", other)));
                }
            };

            let width = ve.width as u32;
            let height = ve.height as u32;

            // Extract extra_data (avcC bytes for H.264) and NAL length size.
            let (extra_data, nal_length_size) = match &ve.codec_specific {
                mp4parse::VideoCodecSpecific::AVCConfig(avcc_bytes) => {
                    let nal_len_size = if avcc_bytes.len() >= 5 {
                        (avcc_bytes[4] & 0x03) + 1
                    } else {
                        4
                    };
                    (avcc_bytes.to_vec(), nal_len_size)
                }
                _ => (Vec::new(), 4),
            };

            return Ok((codec, width, height, extra_data, nal_length_size));
        }
    }
    Err(VideoError::Demux("no video sample entry found".into()))
}

/// Build a flat sample table from the track's stbl boxes.
///
/// Uses stsc (sample-to-chunk), stsz (sample sizes), stco (chunk offsets),
/// stss (sync samples), stts (time-to-sample), and ctts (composition offsets).
fn build_sample_table(track: &mp4parse::Track, _timescale: u64) -> Result<Vec<SampleEntry>> {
    let stco = track
        .stco
        .as_ref()
        .ok_or_else(|| VideoError::Demux("missing stco/co64 box".into()))?;
    let stsz = track
        .stsz
        .as_ref()
        .ok_or_else(|| VideoError::Demux("missing stsz box".into()))?;
    let stsc = track
        .stsc
        .as_ref()
        .ok_or_else(|| VideoError::Demux("missing stsc box".into()))?;
    let stts = track
        .stts
        .as_ref()
        .ok_or_else(|| VideoError::Demux("missing stts box".into()))?;

    // Total number of samples.
    let total_samples = if stsz.sample_size > 0 {
        // Fixed-size samples: count is in sample_sizes length or derived from stts.
        // When sample_size > 0, sample_sizes contains the count as its length,
        // but mp4parse may store the count differently. Use stts to count.
        stts.samples.iter().map(|e| e.sample_count as usize).sum()
    } else {
        stsz.sample_sizes.len()
    };

    // Build sync sample set (1-indexed in stss).
    let sync_set: std::collections::HashSet<u32> = track
        .stss
        .as_ref()
        .map(|stss| stss.samples.iter().copied().collect())
        .unwrap_or_default();
    // If stss is absent, all samples are sync samples.
    let all_sync = track.stss.is_none();

    // Build composition time offset table.
    let ctts_entries: Vec<(u32, i64)> = track
        .ctts
        .as_ref()
        .map(|ctts| {
            ctts.samples
                .iter()
                .map(|e| {
                    let offset = match e.time_offset {
                        mp4parse::TimeOffsetVersion::Version0(v) => v as i64,
                        mp4parse::TimeOffsetVersion::Version1(v) => v as i64,
                    };
                    (e.sample_count, offset)
                })
                .collect()
        })
        .unwrap_or_default();

    // Expand stsc into per-chunk sample counts.
    // stsc entries: (first_chunk [1-indexed], samples_per_chunk, sample_desc_index)
    let num_chunks = stco.offsets.len();
    let mut chunk_sample_counts = vec![0u32; num_chunks];
    let stsc_entries: &[mp4parse::SampleToChunk] = &stsc.samples;

    for (i, chunk_count) in chunk_sample_counts.iter_mut().enumerate() {
        let chunk_1based = (i + 1) as u32;
        // Find which stsc entry applies: the last entry where first_chunk <= chunk_1based.
        let mut spc = 0u32;
        for entry in stsc_entries.iter() {
            if entry.first_chunk <= chunk_1based {
                spc = entry.samples_per_chunk;
            } else {
                break;
            }
        }
        *chunk_count = spc;
    }

    // Build flat sample list.
    let mut samples = Vec::with_capacity(total_samples);
    let mut sample_idx: usize = 0; // 0-indexed global sample counter
    let mut dts_ticks: u64 = 0;

    // Iterators for stts and ctts run-length decoding.
    let mut stts_iter = stts.samples.iter();
    let mut stts_remaining = 0u32;
    let mut stts_delta = 0u32;

    let mut ctts_iter = ctts_entries.iter();
    let mut ctts_remaining = 0u32;
    let mut ctts_offset: i64 = 0;

    for (chunk_idx, &chunk_offset) in stco.offsets.iter().enumerate() {
        let spc = chunk_sample_counts[chunk_idx];
        let mut offset_in_chunk: u64 = 0;

        for _ in 0..spc {
            if sample_idx >= total_samples {
                break;
            }

            // Sample size.
            let size = if stsz.sample_size > 0 {
                stsz.sample_size
            } else if sample_idx < stsz.sample_sizes.len() {
                stsz.sample_sizes[sample_idx]
            } else {
                break;
            };

            // Advance stts.
            if stts_remaining == 0 {
                if let Some(entry) = stts_iter.next() {
                    stts_remaining = entry.sample_count;
                    stts_delta = entry.sample_delta;
                }
            }

            // Advance ctts.
            if ctts_remaining == 0 {
                if let Some(&(count, off)) = ctts_iter.next() {
                    ctts_remaining = count;
                    ctts_offset = off;
                }
            }

            let sample_number_1based = (sample_idx + 1) as u32;
            let is_sync = all_sync || sync_set.contains(&sample_number_1based);

            samples.push(SampleEntry {
                offset: chunk_offset + offset_in_chunk,
                size,
                dts_ticks,
                cts_offset: ctts_offset,
                is_sync,
            });

            offset_in_chunk += size as u64;
            dts_ticks += stts_delta as u64;
            stts_remaining = stts_remaining.saturating_sub(1);
            ctts_remaining = ctts_remaining.saturating_sub(1);
            sample_idx += 1;
        }
    }

    Ok(samples)
}

/// Convert AVCC length-prefixed NAL units to Annex B start-code format.
/// Each NAL in AVCC is prefixed with `nal_length_size` bytes of big-endian length.
/// We replace them with the 4-byte start code 0x00_00_00_01.
fn avcc_to_annex_b(avcc_data: &[u8], nal_length_size: u8, out: &mut Vec<u8>) {
    let len_size = nal_length_size as usize;
    let mut pos = 0;
    while pos + len_size <= avcc_data.len() {
        let mut nal_len: usize = 0;
        for i in 0..len_size {
            nal_len = (nal_len << 8) | (avcc_data[pos + i] as usize);
        }
        pos += len_size;

        if pos + nal_len > avcc_data.len() {
            break;
        }

        out.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        out.extend_from_slice(&avcc_data[pos..pos + nal_len]);
        pos += nal_len;
    }
}

/// Extract SPS and PPS NAL units from the avcC box extra_data and write them
/// in Annex B format (with start codes) into `out`.
fn annex_b_from_avcc_extra(extra: &[u8], out: &mut Vec<u8>) {
    if extra.len() < 7 {
        return;
    }
    // avcC structure:
    //   [0] configurationVersion
    //   [1] AVCProfileIndication
    //   [2] profile_compatibility
    //   [3] AVCLevelIndication
    //   [4] (nal_length_size - 1) & 0x03  (lower 2 bits)
    //   [5] numSPS & 0x1F (lower 5 bits)
    //   then for each SPS: 2-byte length (big-endian) + SPS NAL bytes
    //   then 1 byte: numPPS
    //   then for each PPS: 2-byte length (big-endian) + PPS NAL bytes

    let mut pos = 5;
    let num_sps = (extra[pos] & 0x1F) as usize;
    pos += 1;

    for _ in 0..num_sps {
        if pos + 2 > extra.len() {
            return;
        }
        let sps_len = ((extra[pos] as usize) << 8) | (extra[pos + 1] as usize);
        pos += 2;
        if pos + sps_len > extra.len() {
            return;
        }
        out.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        out.extend_from_slice(&extra[pos..pos + sps_len]);
        pos += sps_len;
    }

    if pos >= extra.len() {
        return;
    }
    let num_pps = extra[pos] as usize;
    pos += 1;

    for _ in 0..num_pps {
        if pos + 2 > extra.len() {
            return;
        }
        let pps_len = ((extra[pos] as usize) << 8) | (extra[pos + 1] as usize);
        pos += 2;
        if pos + pps_len > extra.len() {
            return;
        }
        out.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        out.extend_from_slice(&extra[pos..pos + pps_len]);
        pos += pps_len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path() -> String {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/big_buck_bunny_360p.mp4");
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn open_nonexistent_returns_error() {
        let result = Mp4Demuxer::new("/nonexistent/video.mp4");
        assert!(result.is_err());
    }

    #[test]
    fn create_demuxer_unsupported_ext() {
        let result = crate::demux::create_demuxer("video.avi");
        match result {
            Err(VideoError::UnsupportedCodec(msg)) => {
                assert!(msg.contains(".avi"));
            }
            other => panic!("expected UnsupportedCodec, got {:?}", other.err()),
        }
    }

    #[test]
    fn open_big_buck_bunny_metadata() {
        let path = fixture_path();
        if !std::path::Path::new(&path).exists() {
            eprintln!("Skipping: fixture not found at {}", path);
            return;
        }
        let demuxer = Mp4Demuxer::new(&path).expect("should open");
        let params = demuxer.parameters();
        assert_eq!(params.codec, Codec::H264);
        assert_eq!(params.width, 640);
        assert_eq!(params.height, 360);
        assert!((params.fps - 30.0).abs() < 1.0);
        assert!(params.duration.as_secs() >= 9); // ~10s
    }

    #[test]
    fn packets_are_dts_ordered() {
        let path = fixture_path();
        if !std::path::Path::new(&path).exists() {
            return;
        }
        let mut demuxer = Mp4Demuxer::new(&path).unwrap();
        let mut last_dts = std::time::Duration::ZERO;
        let mut count = 0;
        while let Some(pkt) = demuxer.next_packet().unwrap() {
            assert!(
                pkt.dts >= last_dts,
                "DTS not monotonic at packet {}",
                count
            );
            last_dts = pkt.dts;
            count += 1;
        }
        assert!(count > 100, "expected >100 packets, got {}", count);
    }

    #[test]
    fn first_packet_is_keyframe() {
        let path = fixture_path();
        if !std::path::Path::new(&path).exists() {
            return;
        }
        let mut demuxer = Mp4Demuxer::new(&path).unwrap();
        let pkt = demuxer
            .next_packet()
            .unwrap()
            .expect("should have at least one packet");
        assert!(pkt.is_keyframe);
    }

    #[test]
    fn seek_resets_position() {
        let path = fixture_path();
        if !std::path::Path::new(&path).exists() {
            return;
        }
        let mut demuxer = Mp4Demuxer::new(&path).unwrap();
        // Read a few packets to advance
        for _ in 0..30 {
            demuxer.next_packet().unwrap();
        }
        // Seek to near the beginning
        demuxer.seek(std::time::Duration::from_millis(100)).unwrap();
        let pkt = demuxer
            .next_packet()
            .unwrap()
            .expect("should have packet after seek");
        assert!(pkt.is_keyframe, "seek should land on a keyframe");
    }
}
