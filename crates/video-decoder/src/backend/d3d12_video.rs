//! D3D12 Video API decoder backend (Windows only).

#[cfg(target_os = "windows")]
use std::time::Duration;

#[cfg(target_os = "windows")]
use crate::error::{Result, VideoError};
#[cfg(target_os = "windows")]
use crate::session::{OutputTarget, SessionConfig, VideoSession};
#[cfg(target_os = "windows")]
use crate::types::*;

/// D3D12 Video API decoder session (Windows primary path).
///
/// Uses ID3D12VideoDecoder + ID3D12VideoDecodeCommandList for
/// D3D12-internal HW decode. Shares demux/NAL/DPB logic with
/// Vulkan Video backend.
#[cfg(target_os = "windows")]
pub struct D3d12VideoSession {
    // TODO: Implement when building on Windows
    // demuxer: Box<dyn Demuxer>,
    // video_device: windows::Win32::Graphics::Direct3D12::ID3D12VideoDevice,
    // decoder: windows::Win32::Graphics::Direct3D12::ID3D12VideoDecoder,
    // ... (see design doc §8.2)
    _placeholder: (),
}

#[cfg(target_os = "windows")]
impl D3d12VideoSession {
    pub fn new(_path: &str, _output: &OutputTarget, _config: &SessionConfig) -> Result<Self> {
        // Phase 6 stub - to be implemented on Windows
        Err(VideoError::Decode("D3D12 Video not yet implemented".into()))
    }

    /// Runtime detection: ID3D12Device::QueryInterface(IID_ID3D12VideoDevice).
    pub fn is_supported(_device: *mut std::ffi::c_void) -> bool {
        false
    }
}

#[cfg(target_os = "windows")]
impl VideoSession for D3d12VideoSession {
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
        Backend::D3d12Video
    }
}
