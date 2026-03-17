//! Media Foundation decoder backend (Windows fallback path).

#[cfg(target_os = "windows")]
use std::time::Duration;

#[cfg(target_os = "windows")]
use crate::error::{Result, VideoError};
#[cfg(target_os = "windows")]
use crate::session::{OutputTarget, SessionConfig, VideoSession};
#[cfg(target_os = "windows")]
use crate::types::*;

/// Media Foundation decoder session (Windows fallback).
///
/// Uses IMFTransform (MFT) for HW-accelerated decode via DXVA2/D3D11.
/// Preferred when D3D12 Video is not available (older GPUs, D3D11-only
/// wgpu backend, etc.).
#[cfg(target_os = "windows")]
pub struct MfVideoSession {
    // TODO: Implement when building on Windows
    // demuxer: Box<dyn Demuxer>,
    // mf_transform: windows::Win32::Media::MediaFoundation::IMFTransform,
    // d3d11_device: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    _placeholder: (),
}

#[cfg(target_os = "windows")]
impl MfVideoSession {
    pub fn new(_path: &str, _output: &OutputTarget, _config: &SessionConfig) -> Result<Self> {
        // Phase 6 stub - to be implemented on Windows
        Err(VideoError::Decode(
            "Media Foundation not yet implemented".into(),
        ))
    }
}

#[cfg(target_os = "windows")]
impl VideoSession for MfVideoSession {
    fn info(&self) -> &VideoInfo {
        unreachable!()
    }
    fn position(&self) -> Duration {
        unreachable!()
    }
    fn decode_frame(&mut self, _dt: Duration) -> Result<FrameStatus> {
        unreachable!()
    }
    fn seek(&mut self, _position: Duration) -> Result<()> {
        unreachable!()
    }
    fn set_looping(&mut self, _looping: bool) {
        unreachable!()
    }
    fn is_looping(&self) -> bool {
        unreachable!()
    }
    fn pause(&mut self) {
        unreachable!()
    }
    fn resume(&mut self) {
        unreachable!()
    }
    fn is_paused(&self) -> bool {
        unreachable!()
    }
    fn backend(&self) -> Backend {
        Backend::MediaFoundation
    }
}
