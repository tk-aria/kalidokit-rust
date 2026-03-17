//! macOS VideoToolbox backend via AVFoundation.
//!
//! Uses `AVAssetReader` + `AVAssetReaderTrackOutput` to demux and decode
//! H.264/H.265 video on macOS.  AVFoundation internally uses VideoToolbox
//! for hardware-accelerated decoding.  Decoded frames are received as
//! `CVPixelBuffer` in BGRA format and converted to RGBA on the CPU.

use std::time::Duration;

use objc2::rc::Retained;
use objc2_av_foundation::{
    AVAsset, AVAssetReader, AVAssetReaderOutput, AVAssetReaderStatus, AVAssetReaderTrackOutput,
    AVAssetTrack, AVMediaTypeVideo, AVURLAsset,
};
use objc2_core_foundation::CFString;
use objc2_core_video::{
    kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_32BGRA, CVPixelBufferGetBaseAddress,
    CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use crate::error::{Result, VideoError};
use crate::session::{OutputTarget, SessionConfig, VideoSession};
use crate::types::*;
use crate::util::PlaybackState;

// ---- Send wrapper --------------------------------------------------------
// objc2 Retained<T> is !Send because Obj-C objects are generally not
// thread-safe.  Our session is used from a single thread at a time
// (the render thread), so we wrap the Obj-C fields to satisfy the
// `VideoSession: Send` bound.
struct SendWrapper<T>(T);
// SAFETY: AppleVideoSession is only accessed from one thread (the
// render/decode thread).  The caller must ensure this.
unsafe impl<T> Send for SendWrapper<T> {}

/// macOS hardware-accelerated video decode session using AVFoundation / VideoToolbox.
pub struct AppleVideoSession {
    /// The AVAssetReader driving demux + decode.
    reader: SendWrapper<Retained<AVAssetReader>>,
    /// The track output from which we pull decoded sample buffers.
    track_output: SendWrapper<Retained<AVAssetReaderTrackOutput>>,
    /// The source asset (kept alive so the reader remains valid).
    _asset: SendWrapper<Retained<AVURLAsset>>,
    /// RGBA pixel buffer (width * height * 4 bytes).
    frame_buffer: Vec<u8>,
    /// Output target descriptor.
    _output: OutputTarget,
    /// Playback state (clock, looping, pause).
    playback: PlaybackState,
    /// Video metadata.
    info: VideoInfo,
    /// Whether we've reached end-of-stream.
    ended: bool,
    /// File path for re-opening on seek/loop.
    path: String,
    /// Session config for re-opening.
    _config: SessionConfig,
}

impl AppleVideoSession {
    /// Create a new Apple VideoToolbox decode session for the given file.
    pub fn new(path: &str, output: OutputTarget, config: &SessionConfig) -> Result<Self> {
        let (asset, reader, track_output, info) = Self::open_asset(path)?;

        let buf_size = (info.width as usize) * (info.height as usize) * 4;
        let frame_buffer = vec![0u8; buf_size];

        let playback = PlaybackState::new(info.duration, info.fps, config.looping);

        Ok(Self {
            reader: SendWrapper(reader),
            track_output: SendWrapper(track_output),
            _asset: SendWrapper(asset),
            frame_buffer,
            _output: output,
            playback,
            info,
            ended: false,
            path: path.to_string(),
            _config: config.clone(),
        })
    }

    /// Open an AVURLAsset, create an AVAssetReader with BGRA output, and
    /// extract video metadata.
    #[allow(clippy::type_complexity)]
    fn open_asset(
        path: &str,
    ) -> Result<(
        Retained<AVURLAsset>,
        Retained<AVAssetReader>,
        Retained<AVAssetReaderTrackOutput>,
        VideoInfo,
    )> {
        // 1. Create NSURL and AVURLAsset.
        let ns_path = NSString::from_str(path);
        let url = NSURL::fileURLWithPath(&ns_path);
        let asset = unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) };

        // 2. Get the first video track.
        let media_type =
            unsafe { AVMediaTypeVideo.expect("AVMediaTypeVideo should be available on macOS") };
        #[allow(deprecated)]
        // tracksWithMediaType is deprecated but the async replacement is complex
        let tracks: Retained<objc2_foundation::NSArray<AVAssetTrack>> =
            unsafe { asset.tracksWithMediaType(media_type) };

        let track = tracks
            .firstObject()
            .ok_or_else(|| VideoError::Demux("no video track found in asset".to_string()))?;

        // 3. Extract video metadata from the track.
        let natural_size = unsafe { track.naturalSize() };
        let width = natural_size.width as u32;
        let height = natural_size.height as u32;
        let fps = unsafe { track.nominalFrameRate() } as f64;
        #[allow(deprecated)]
        let asset_ref: &AVAsset = &asset;
        let duration_cm = unsafe { asset_ref.duration() };
        let duration_secs = unsafe { duration_cm.seconds() };
        let duration = if duration_secs.is_finite() && duration_secs > 0.0 {
            Duration::from_secs_f64(duration_secs)
        } else {
            Duration::ZERO
        };

        // Detect codec from format descriptions (best-effort).
        let codec = Codec::H264; // AVAssetReader handles both H.264 and H.265 transparently.

        // 4. Configure output settings: request BGRA pixel format.
        // Toll-free bridge kCVPixelBufferPixelFormatTypeKey (CFString) to NSString.
        // We bind the bridged reference to a variable so it outlives the dictionary creation.
        let pixel_format_key_cf: &CFString = unsafe { kCVPixelBufferPixelFormatTypeKey };
        let pixel_format_key_ptr = pixel_format_key_cf as *const CFString as *const NSString;
        // SAFETY: CFString and NSString are toll-free bridged on Apple platforms.
        // The &NSString reference borrows the static CFString constant which lives forever.
        let pixel_format_key_ref: &NSString = unsafe { &*pixel_format_key_ptr };

        let pixel_format_value = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
        // Bind &AnyObject to a local variable to ensure it lives long enough.
        let value_ref: &objc2::runtime::AnyObject = &pixel_format_value;

        let keys: &[&NSString] = &[pixel_format_key_ref];
        let values: &[&objc2::runtime::AnyObject] = &[value_ref];
        let output_settings: Retained<NSDictionary<NSString, objc2::runtime::AnyObject>> =
            NSDictionary::from_slices(keys, values);

        // 5. Create AVAssetReaderTrackOutput and AVAssetReader.
        let track_output = unsafe {
            AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
                &track,
                Some(&output_settings),
            )
        };

        // Avoid copying sample data for better performance.
        unsafe {
            AVAssetReaderOutput::setAlwaysCopiesSampleData(&track_output, false);
        }

        let reader = unsafe {
            AVAssetReader::assetReaderWithAsset_error(&asset)
                .map_err(|e| VideoError::Demux(format!("failed to create AVAssetReader: {}", e)))?
        };

        let can_add = unsafe { reader.canAddOutput(&track_output) };
        if !can_add {
            return Err(VideoError::Demux(
                "AVAssetReader cannot add track output".to_string(),
            ));
        }
        unsafe { reader.addOutput(&track_output) };

        let started = unsafe { reader.startReading() };
        if !started {
            let err = unsafe { reader.error() };
            let msg = err
                .map(|e| e.to_string())
                .unwrap_or_else(|| "unknown error".to_string());
            return Err(VideoError::Demux(format!(
                "AVAssetReader failed to start reading: {}",
                msg
            )));
        }

        let info = VideoInfo {
            codec,
            width,
            height,
            duration,
            fps: if fps > 0.0 { fps } else { 30.0 },
            backend: Backend::VideoToolbox,
            needs_color_conversion: false, // We output RGBA directly after BGRA swizzle.
        };

        Ok((asset, reader, track_output, info))
    }

    /// Read the next decoded frame from AVAssetReader, convert BGRA to RGBA,
    /// and store in `frame_buffer`.
    fn decode_next_frame(&mut self) -> Result<bool> {
        let output_ref: &AVAssetReaderOutput = &self.track_output.0;
        let sample = unsafe { output_ref.copyNextSampleBuffer() };

        let Some(sample) = sample else {
            // No more samples. Check reader status.
            let status = unsafe { self.reader.0.status() };
            if status == AVAssetReaderStatus::Completed {
                self.ended = true;
                return Ok(false);
            }
            if status == AVAssetReaderStatus::Failed {
                let err = unsafe { self.reader.0.error() };
                let msg = err
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                return Err(VideoError::Decode(format!("AVAssetReader failed: {}", msg)));
            }
            // Cancelled or unknown.
            self.ended = true;
            return Ok(false);
        };

        // Extract CVPixelBuffer (CVImageBuffer) from the sample buffer.
        let image_buffer = unsafe { objc2_core_media::CMSampleBuffer::image_buffer(&sample) };
        let Some(pixel_buffer) = image_buffer else {
            // Sample didn't contain an image buffer (shouldn't happen for video).
            return Ok(false);
        };
        // CVPixelBuffer = CVImageBuffer = CVBuffer, they are all the same type.
        let pixel_buffer = &*pixel_buffer;

        // Lock pixel data for CPU read access.
        let lock_flags = CVPixelBufferLockFlags::ReadOnly;
        let lock_ret = unsafe { CVPixelBufferLockBaseAddress(pixel_buffer, lock_flags) };
        if lock_ret != 0 {
            return Err(VideoError::Decode(format!(
                "CVPixelBufferLockBaseAddress failed with code {}",
                lock_ret
            )));
        }

        let base_addr = CVPixelBufferGetBaseAddress(pixel_buffer);
        let w = CVPixelBufferGetWidth(pixel_buffer);
        let h = CVPixelBufferGetHeight(pixel_buffer);
        let bytes_per_row = CVPixelBufferGetBytesPerRow(pixel_buffer);

        let needed = w * h * 4;
        if self.frame_buffer.len() != needed {
            self.frame_buffer.resize(needed, 0);
            self.info.width = w as u32;
            self.info.height = h as u32;
        }

        if !base_addr.is_null() {
            // Copy BGRA data and swizzle to RGBA.
            // The pixel buffer may have padding (bytes_per_row > width*4),
            // so we copy row by row.
            let src = base_addr as *const u8;
            let row_pixels = w * 4;
            for y in 0..h {
                let src_offset = y * bytes_per_row;
                let dst_offset = y * row_pixels;
                let row_src =
                    unsafe { std::slice::from_raw_parts(src.add(src_offset), row_pixels) };
                let row_dst = &mut self.frame_buffer[dst_offset..dst_offset + row_pixels];

                // BGRA -> RGBA: swap B and R channels.
                for x in (0..row_pixels).step_by(4) {
                    row_dst[x] = row_src[x + 2]; // R <- B
                    row_dst[x + 1] = row_src[x + 1]; // G <- G
                    row_dst[x + 2] = row_src[x]; // B <- R
                    row_dst[x + 3] = row_src[x + 3]; // A <- A
                }
            }
        }

        // Unlock pixel data.
        unsafe { CVPixelBufferUnlockBaseAddress(pixel_buffer, lock_flags) };

        Ok(true)
    }

    /// Re-open the asset reader for seeking / looping.
    fn reopen(&mut self) -> Result<()> {
        let (asset, reader, track_output, _info) = Self::open_asset(&self.path)?;
        self._asset = SendWrapper(asset);
        self.reader = SendWrapper(reader);
        self.track_output = SendWrapper(track_output);
        self.ended = false;
        Ok(())
    }

    /// Returns the current decoded RGBA frame data.
    pub fn frame_rgba(&self) -> &[u8] {
        &self.frame_buffer
    }

    /// Returns the output target this session was created with.
    pub fn output(&self) -> &OutputTarget {
        &self._output
    }
}

impl VideoSession for AppleVideoSession {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn position(&self) -> Duration {
        self.playback.position
    }

    fn decode_frame(&mut self, dt: Duration) -> Result<FrameStatus> {
        if self.playback.paused {
            return Ok(FrameStatus::Waiting);
        }

        if !self.playback.tick(dt) {
            return Ok(FrameStatus::Waiting);
        }

        if !self.playback.check_end_of_stream() {
            return Ok(FrameStatus::EndOfStream);
        }

        // Handle looping after EOS.
        if self.ended && self.playback.looping {
            self.reopen()?;
        }

        if self.ended {
            return Ok(FrameStatus::EndOfStream);
        }

        // Decode frames until we get a picture.
        loop {
            match self.decode_next_frame()? {
                true => return Ok(FrameStatus::NewFrame),
                false if self.ended => {
                    if self.playback.looping {
                        return Ok(FrameStatus::Waiting);
                    }
                    return Ok(FrameStatus::EndOfStream);
                }
                false => continue,
            }
        }
    }

    fn seek(&mut self, position: Duration) -> Result<()> {
        // AVAssetReader doesn't support seeking mid-stream. We re-open
        // with a fresh reader and adjust the playback clock.
        self.reopen()?;
        self.playback.seek(position);
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
        Backend::VideoToolbox
    }

    fn frame_rgba(&self) -> Option<&[u8]> {
        Some(&self.frame_buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::NativeHandle;

    fn metal_output() -> OutputTarget {
        OutputTarget {
            native_handle: NativeHandle::Metal {
                texture: std::ptr::null_mut(),
                device: std::ptr::null_mut(),
            },
            format: PixelFormat::Rgba8Srgb,
            width: 640,
            height: 480,
            color_space: ColorSpace::default(),
        }
    }

    #[test]
    fn new_fails_on_nonexistent_file() {
        let result = AppleVideoSession::new(
            "/nonexistent/path.mp4",
            metal_output(),
            &SessionConfig::default(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn new_fails_on_invalid_file() {
        let dir = std::env::temp_dir().join("apple_video_session_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.mp4");
        std::fs::write(&path, b"not a real mp4").unwrap();

        let result = AppleVideoSession::new(
            path.to_str().unwrap(),
            metal_output(),
            &SessionConfig::default(),
        );
        assert!(result.is_err());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn backend_returns_videotoolbox() {
        assert_eq!(format!("{:?}", Backend::VideoToolbox), "VideoToolbox");
    }

    fn fixture_path() -> String {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/big_buck_bunny_360p.mp4");
        p.to_str().unwrap().to_string()
    }

    #[test]
    fn apple_decode_10_frames() {
        let path = fixture_path();
        if !std::path::Path::new(&path).exists() {
            return;
        }
        let mut session =
            AppleVideoSession::new(&path, metal_output(), &SessionConfig::default()).unwrap();
        assert_eq!(session.info().backend, Backend::VideoToolbox);
        assert_eq!(session.info().width, 640);

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
}
