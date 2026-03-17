//! Backend selection and session creation.

#[cfg(target_os = "macos")]
pub mod apple;
pub mod software;

use crate::error::{Result, VideoError};
use crate::handle::NativeHandle;
use crate::session::{OutputTarget, SessionConfig, VideoSession};
use crate::types::Backend;

#[cfg(target_os = "macos")]
use self::apple::AppleVideoSession;
use self::software::SwVideoSession;

/// Create a video session with automatic backend selection.
///
/// The selection strategy is:
/// 1. If `config.preferred_backend` is set, try that backend only.
/// 2. Otherwise, detect candidate backends from the `NativeHandle` variant.
/// 3. Try each candidate in order; on failure, fall through to the next.
/// 4. If all candidates fail and `config.allow_software_fallback` is true,
///    try the software (openh264) backend as a last resort.
pub fn create_session(
    path: &str,
    output: OutputTarget,
    config: SessionConfig,
) -> Result<Box<dyn VideoSession>> {
    if !std::path::Path::new(path).exists() {
        return Err(VideoError::FileNotFound(path.to_string()));
    }

    // Explicit backend requested.
    if let Some(backend) = config.preferred_backend {
        return create_with_backend(path, &output, &config, backend);
    }

    // Auto-detect candidates from native handle.
    let candidates = detect_backends(&output.native_handle);

    for backend in &candidates {
        match create_with_backend(path, &output, &config, *backend) {
            Ok(session) => return Ok(session),
            Err(e) => {
                log::warn!("Backend {:?} failed: {}, trying next", backend, e);
            }
        }
    }

    // Software fallback.
    if config.allow_software_fallback {
        return create_with_backend(path, &output, &config, Backend::Software);
    }

    Err(VideoError::NoHwDecoder)
}

/// Determine candidate backends for the given native handle, ordered by preference.
fn detect_backends(handle: &NativeHandle) -> Vec<Backend> {
    match handle {
        NativeHandle::Metal { .. } => vec![Backend::VideoToolbox],
        NativeHandle::D3d12 { .. } => vec![Backend::D3d12Video, Backend::MediaFoundation],
        NativeHandle::D3d11 { .. } => vec![Backend::MediaFoundation],
        NativeHandle::Vulkan { .. } => vec![Backend::VulkanVideo, Backend::GStreamerVaapi],
        // Wgpu handle has no HW-accelerated path; go straight to software.
        NativeHandle::Wgpu { .. } => vec![Backend::Software],
    }
}

/// Try to create a session with a specific backend.
fn create_with_backend(
    path: &str,
    output: &OutputTarget,
    config: &SessionConfig,
    backend: Backend,
) -> Result<Box<dyn VideoSession>> {
    match backend {
        Backend::Software => {
            let session = SwVideoSession::new(path, *output, config)?;
            Ok(Box::new(session))
        }
        #[cfg(target_os = "macos")]
        Backend::VideoToolbox => {
            let session = AppleVideoSession::new(path, *output, config)?;
            Ok(Box::new(session))
        }
        // HW backends not yet implemented (or not available on this platform).
        _ => Err(VideoError::NoHwDecoder),
    }
}
