//! # ten-vad
//!
//! Rust bindings for [TEN VAD](https://github.com/TEN-framework/ten-vad),
//! a low-latency, high-performance voice activity detector.
//!
//! Uses prebuilt native libraries — no C/C++ compilation required.
//!
//! ## Example
//!
//! ```rust,no_run
//! use ten_vad::{TenVad, HopSize};
//!
//! let mut vad = TenVad::new(HopSize::Samples256, 0.5).unwrap();
//! println!("TEN VAD {}", TenVad::version());
//!
//! let silence = vec![0i16; 256];
//! let result = vad.process(&silence).unwrap();
//! println!("voice={} prob={:.3}", result.is_voice, result.probability);
//! ```

pub mod ffi;

use std::fmt;

// ── Types ────────────────────────────────────────────────────

/// Supported hop (frame) sizes for 16 kHz audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HopSize {
    /// 160 samples = 10 ms at 16 kHz.
    Samples160 = 160,
    /// 256 samples = 16 ms at 16 kHz.
    Samples256 = 256,
}

impl HopSize {
    /// Number of samples per frame.
    pub fn as_usize(self) -> usize {
        self as usize
    }
}

/// Voice activity detection result for one audio frame.
#[derive(Debug, Clone, Copy)]
pub struct VadResult {
    /// Probability of voice presence in [0.0, 1.0].
    pub probability: f32,
    /// `true` when probability ≥ threshold.
    pub is_voice: bool,
}

/// Errors from VAD operations.
#[derive(Debug)]
pub enum VadError {
    /// `ten_vad_create` failed.
    CreateFailed,
    /// `ten_vad_process` failed.
    ProcessFailed,
    /// Audio frame length ≠ configured hop size.
    InvalidFrameSize { expected: usize, actual: usize },
    /// Threshold outside [0.0, 1.0].
    InvalidThreshold(f32),
}

impl fmt::Display for VadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateFailed => write!(f, "ten_vad_create failed"),
            Self::ProcessFailed => write!(f, "ten_vad_process failed"),
            Self::InvalidFrameSize { expected, actual } => {
                write!(f, "frame size: expected {expected}, got {actual}")
            }
            Self::InvalidThreshold(t) => {
                write!(f, "threshold {t} outside [0.0, 1.0]")
            }
        }
    }
}

impl std::error::Error for VadError {}

// ── TenVad ───────────────────────────────────────────────────

/// A TEN VAD voice activity detector.
///
/// Wraps the native C handle. Destroyed automatically on [`Drop`].
///
/// # Thread Safety
/// `Send` but not `Sync` — move between threads but do not share.
pub struct TenVad {
    handle: ffi::TenVadHandle,
    hop_size: HopSize,
}

impl TenVad {
    /// Create a new detector.
    ///
    /// - `hop_size`: `Samples160` (10 ms) or `Samples256` (16 ms) at 16 kHz.
    /// - `threshold`: Sensitivity in [0.0, 1.0]. Lower = more aggressive.
    pub fn new(hop_size: HopSize, threshold: f32) -> Result<Self, VadError> {
        if !(0.0..=1.0).contains(&threshold) {
            return Err(VadError::InvalidThreshold(threshold));
        }
        let mut handle: ffi::TenVadHandle = std::ptr::null_mut();
        let ret = unsafe { ffi::ten_vad_create(&mut handle, hop_size.as_usize(), threshold) };
        if ret != 0 || handle.is_null() {
            return Err(VadError::CreateFailed);
        }
        Ok(Self { handle, hop_size })
    }

    /// Process one frame of 16 kHz 16-bit PCM audio.
    ///
    /// `audio` must contain exactly [`hop_size`](Self::hop_size) samples.
    pub fn process(&mut self, audio: &[i16]) -> Result<VadResult, VadError> {
        let expected = self.hop_size.as_usize();
        if audio.len() != expected {
            return Err(VadError::InvalidFrameSize {
                expected,
                actual: audio.len(),
            });
        }
        let mut prob: f32 = 0.0;
        let mut flag: i32 = 0;
        let ret = unsafe {
            ffi::ten_vad_process(
                self.handle,
                audio.as_ptr(),
                audio.len(),
                &mut prob,
                &mut flag,
            )
        };
        if ret != 0 {
            return Err(VadError::ProcessFailed);
        }
        Ok(VadResult {
            probability: prob,
            is_voice: flag != 0,
        })
    }

    /// Configured hop size.
    pub fn hop_size(&self) -> HopSize {
        self.hop_size
    }

    /// Library version (e.g. `"1.0.0"`).
    pub fn version() -> &'static str {
        let ptr = unsafe { ffi::ten_vad_get_version() };
        if ptr.is_null() {
            return "unknown";
        }
        unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_str()
            .unwrap_or("unknown")
    }
}

impl Drop for TenVad {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { ffi::ten_vad_destroy(&mut self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

unsafe impl Send for TenVad {}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_below_zero() {
        assert!(matches!(
            TenVad::new(HopSize::Samples256, -0.1),
            Err(VadError::InvalidThreshold(_))
        ));
    }

    #[test]
    fn threshold_above_one() {
        assert!(matches!(
            TenVad::new(HopSize::Samples256, 1.1),
            Err(VadError::InvalidThreshold(_))
        ));
    }

    #[test]
    fn wrong_frame_length() {
        if let Ok(mut vad) = TenVad::new(HopSize::Samples256, 0.5) {
            let short = vec![0i16; 100];
            match vad.process(&short) {
                Err(VadError::InvalidFrameSize {
                    expected: 256,
                    actual: 100,
                }) => {}
                other => panic!("expected InvalidFrameSize, got {other:?}"),
            }
        }
    }

    #[test]
    fn silence_is_not_voice() {
        if let Ok(mut vad) = TenVad::new(HopSize::Samples256, 0.5) {
            let silence = vec![0i16; 256];
            let r = vad.process(&silence).unwrap();
            assert!((0.0..=1.0).contains(&r.probability));
            assert!(!r.is_voice, "silence detected as voice");
        }
    }

    #[test]
    fn version_non_empty() {
        let v = TenVad::version();
        assert!(!v.is_empty());
    }

    #[test]
    fn hop_size_values() {
        assert_eq!(HopSize::Samples160.as_usize(), 160);
        assert_eq!(HopSize::Samples256.as_usize(), 256);
    }
}
