//! Software H.264 decoder backend using openh264.
//!
//! Decodes H.264 NAL units on the CPU and converts YUV420 to RGBA.
//! The RGBA buffer is stored in memory; GPU texture upload is handled
//! by the caller (or a future integration layer).

use std::time::Duration;

use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

use crate::demux::{create_demuxer, Demuxer};
use crate::error::{Result, VideoError};
use crate::session::{OutputTarget, SessionConfig, VideoSession};
use crate::types::*;
use crate::util::PlaybackState;

/// Software video decode session backed by openh264.
pub struct SwVideoSession {
    demuxer: Box<dyn Demuxer>,
    decoder: Decoder,
    /// RGBA pixel buffer (width * height * 4 bytes).
    frame_buffer: Vec<u8>,
    output: OutputTarget,
    playback: PlaybackState,
    info: VideoInfo,
    ended: bool,
}

impl SwVideoSession {
    /// Create a new software decode session for the given file.
    pub fn new(path: &str, output: OutputTarget, config: &SessionConfig) -> Result<Self> {
        let demuxer = create_demuxer(path)?;
        let params = demuxer.parameters();

        if params.codec != Codec::H264 {
            return Err(VideoError::UnsupportedCodec(format!(
                "software backend only supports H.264, got {:?}",
                params.codec
            )));
        }

        let decoder = Decoder::new()
            .map_err(|e| VideoError::Decode(format!("failed to create openh264 decoder: {}", e)))?;

        let info = VideoInfo {
            codec: params.codec,
            width: params.width,
            height: params.height,
            duration: params.duration,
            fps: params.fps,
            backend: Backend::Software,
            needs_color_conversion: false, // we output RGBA directly
        };

        let buf_size = (params.width as usize) * (params.height as usize) * 4;
        let frame_buffer = vec![0u8; buf_size];

        let playback = PlaybackState::new(params.duration, params.fps, config.looping);

        Ok(Self {
            demuxer,
            decoder,
            frame_buffer,
            output,
            playback,
            info,
            ended: false,
        })
    }

    /// Decode the next packet from the demuxer and write RGBA into `frame_buffer`.
    fn decode_next_packet(&mut self) -> Result<bool> {
        let packet = self.demuxer.next_packet()?;

        let Some(packet) = packet else {
            self.ended = true;
            return Ok(false);
        };

        let yuv = self
            .decoder
            .decode(&packet.data)
            .map_err(|e| VideoError::Decode(format!("openh264 decode error: {}", e)))?;

        let Some(yuv) = yuv else {
            // Decoder consumed the NAL but hasn't produced a picture yet.
            return Ok(false);
        };

        let (w, h) = yuv.dimensions();
        let needed = w * h * 4;

        if self.frame_buffer.len() != needed {
            self.frame_buffer.resize(needed, 0);
            // Update info dimensions if the stream changed resolution.
            self.info.width = w as u32;
            self.info.height = h as u32;
        }

        yuv.write_rgba8(&mut self.frame_buffer);

        Ok(true)
    }
}

impl VideoSession for SwVideoSession {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn position(&self) -> Duration {
        self.playback.position
    }

    fn decode_frame(&mut self, dt: Duration) -> Result<FrameStatus> {
        // Paused: no progress.
        if self.playback.paused {
            return Ok(FrameStatus::Waiting);
        }

        // Advance playback clock; if not time for a new frame, wait.
        if !self.playback.tick(dt) {
            return Ok(FrameStatus::Waiting);
        }

        // Check end-of-stream / looping.
        if !self.playback.check_end_of_stream() {
            return Ok(FrameStatus::EndOfStream);
        }

        // If we previously hit EOS and looped, the PlaybackState already
        // reset position to 0. We need to seek the demuxer back to the start
        // and reset the decoder.
        if self.ended && self.playback.looping {
            self.demuxer.seek(Duration::ZERO)?;
            // Re-create decoder to flush internal state.
            self.decoder = Decoder::new().map_err(|e| {
                VideoError::Decode(format!("failed to recreate openh264 decoder: {}", e))
            })?;
            self.ended = false;
        }

        if self.ended {
            return Ok(FrameStatus::EndOfStream);
        }

        // Decode. We may need to feed multiple NALs before getting a picture.
        loop {
            match self.decode_next_packet()? {
                true => return Ok(FrameStatus::NewFrame),
                false if self.ended => {
                    // Demuxer exhausted.
                    if self.playback.looping {
                        // Will loop on the next call.
                        return Ok(FrameStatus::Waiting);
                    }
                    return Ok(FrameStatus::EndOfStream);
                }
                false => {
                    // NAL consumed but no picture yet; feed more.
                    continue;
                }
            }
        }
    }

    fn seek(&mut self, position: Duration) -> Result<()> {
        self.demuxer.seek(position)?;
        self.playback.seek(position);
        // Re-create decoder to flush internal state after seek.
        self.decoder = Decoder::new().map_err(|e| {
            VideoError::Decode(format!("failed to recreate openh264 decoder: {}", e))
        })?;
        self.ended = false;
        Ok(())
    }

    fn set_looping(&mut self, looping: bool) {
        self.playback.looping = looping;
    }

    fn is_looping(&self) -> bool {
        self.playback.looping
    }

    fn pause(&mut self) {
        self.playback.pause();
    }

    fn resume(&mut self) {
        self.playback.resume();
    }

    fn is_paused(&self) -> bool {
        self.playback.paused
    }

    fn backend(&self) -> Backend {
        Backend::Software
    }

    fn frame_rgba(&self) -> Option<&[u8]> {
        Some(&self.frame_buffer)
    }
}

/// Return a reference to the current RGBA frame buffer.
impl SwVideoSession {
    /// Returns the current decoded RGBA frame data.
    ///
    /// This is a CPU-side buffer. The caller is responsible for uploading
    /// it to a GPU texture if needed.
    pub fn frame_rgba(&self) -> &[u8] {
        &self.frame_buffer
    }

    /// Returns the output target this session was created with.
    pub fn output(&self) -> &OutputTarget {
        &self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::NativeHandle;

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
    fn new_fails_on_nonexistent_file() {
        let result = SwVideoSession::new(
            "/nonexistent/path.mp4",
            dummy_output(),
            &SessionConfig::default(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn new_fails_on_unsupported_container() {
        let result = SwVideoSession::new("video.avi", dummy_output(), &SessionConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn new_fails_on_invalid_mp4() {
        let dir = std::env::temp_dir().join("sw_video_session_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.mp4");
        std::fs::write(&path, b"not a real mp4").unwrap();

        let result = SwVideoSession::new(
            path.to_str().unwrap(),
            dummy_output(),
            &SessionConfig::default(),
        );
        assert!(result.is_err());

        std::fs::remove_file(&path).ok();
    }

    fn fixture_path() -> String {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/big_buck_bunny_360p.mp4");
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn sw_decode_10_frames() {
        let path = fixture_path();
        if !std::path::Path::new(&path).exists() {
            return;
        }
        let mut session =
            SwVideoSession::new(&path, dummy_output(), &SessionConfig::default()).unwrap();
        assert_eq!(session.info().codec, Codec::H264);
        assert_eq!(session.info().width, 640);
        assert_eq!(session.info().backend, Backend::Software);

        let dt = std::time::Duration::from_secs_f64(1.0 / 30.0);
        let mut new_frames = 0;
        for _ in 0..100 {
            match session.decode_frame(dt).unwrap() {
                FrameStatus::NewFrame => new_frames += 1,
                FrameStatus::Waiting => {}
                FrameStatus::EndOfStream => break,
            }
            if new_frames >= 10 {
                break;
            }
        }
        assert!(new_frames >= 10, "expected >=10 frames, got {}", new_frames);
        assert!(!session.frame_rgba().is_empty());
    }

    #[test]
    fn sw_pause_resume() {
        let path = fixture_path();
        if !std::path::Path::new(&path).exists() {
            return;
        }
        let mut session =
            SwVideoSession::new(&path, dummy_output(), &SessionConfig::default()).unwrap();
        let dt = std::time::Duration::from_secs_f64(1.0 / 30.0);

        session.pause();
        assert!(session.is_paused());
        assert_eq!(session.decode_frame(dt).unwrap(), FrameStatus::Waiting);

        session.resume();
        assert!(!session.is_paused());
        // Should eventually produce a frame
        let mut got_frame = false;
        for _ in 0..10 {
            if session.decode_frame(dt).unwrap() == FrameStatus::NewFrame {
                got_frame = true;
                break;
            }
        }
        assert!(got_frame);
    }
}
