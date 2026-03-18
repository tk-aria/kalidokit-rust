//! Raw FFI bindings to the TEN VAD C library.
//!
//! These declarations correspond exactly to `include/ten_vad.h`.
//! The library is linked at compile time by `build.rs`.

use std::ffi::c_void;
use std::os::raw::{c_char, c_float, c_int};

/// Opaque handle for a TEN VAD instance.
pub type TenVadHandle = *mut c_void;

extern "C" {
    /// Create and initialize a TEN VAD instance.
    ///
    /// # Parameters
    /// - `handle`: Receives the allocated handle on success.
    /// - `hop_size`: Samples per frame (160 or 256 for 16 kHz).
    /// - `threshold`: Detection threshold in [0.0, 1.0].
    ///
    /// # Returns
    /// 0 on success, -1 on failure.
    pub fn ten_vad_create(handle: *mut TenVadHandle, hop_size: usize, threshold: c_float) -> c_int;

    /// Process one audio frame.
    ///
    /// # Parameters
    /// - `handle`: Valid handle from `ten_vad_create`.
    /// - `audio_data`: `hop_size` samples of 16-bit PCM at 16 kHz.
    /// - `audio_data_length`: Must equal `hop_size`.
    /// - `out_probability`: Receives voice probability [0.0, 1.0].
    /// - `out_flag`: Receives 1 (voice) or 0 (no voice).
    ///
    /// # Returns
    /// 0 on success, -1 on failure.
    pub fn ten_vad_process(
        handle: TenVadHandle,
        audio_data: *const i16,
        audio_data_length: usize,
        out_probability: *mut c_float,
        out_flag: *mut c_int,
    ) -> c_int;

    /// Destroy a TEN VAD instance.
    ///
    /// Sets `*handle` to NULL on success.
    ///
    /// # Returns
    /// 0 on success, -1 on failure.
    pub fn ten_vad_destroy(handle: *mut TenVadHandle) -> c_int;

    /// Return the library version string (e.g. "1.0.0").
    ///
    /// The pointer is valid for the lifetime of the process.
    pub fn ten_vad_get_version() -> *const c_char;
}
