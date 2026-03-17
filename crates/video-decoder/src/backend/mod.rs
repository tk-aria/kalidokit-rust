//! Backend selection and session creation.

#[cfg(target_os = "macos")]
pub mod apple;
#[cfg(target_os = "windows")]
pub mod d3d12_video;
#[cfg(all(target_os = "linux", feature = "gstreamer"))]
pub mod gst_vaapi;
#[cfg(target_os = "android")]
pub mod media_codec;
#[cfg(target_os = "windows")]
pub mod media_foundation;
pub mod software;
#[cfg(all(target_os = "linux", feature = "v4l2"))]
pub mod v4l2;
#[cfg(target_os = "linux")]
pub mod vulkan_video;

use crate::error::{Result, VideoError};
use crate::handle::NativeHandle;
use crate::session::{OutputTarget, SessionConfig, VideoSession};
use crate::types::Backend;

#[cfg(target_os = "macos")]
use self::apple::AppleVideoSession;
#[cfg(target_os = "windows")]
use self::d3d12_video::D3d12VideoSession;
#[cfg(all(target_os = "linux", feature = "gstreamer"))]
use self::gst_vaapi::GstVideoSession;
#[cfg(target_os = "android")]
use self::media_codec::McVideoSession;
#[cfg(target_os = "windows")]
use self::media_foundation::MfVideoSession;
use self::software::SwVideoSession;
#[cfg(all(target_os = "linux", feature = "v4l2"))]
use self::v4l2::V4l2VideoSession;
#[cfg(target_os = "linux")]
use self::vulkan_video::VkVideoSession;

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
        NativeHandle::D3d12 { .. } => {
            let mut backends = Vec::new();
            #[cfg(target_os = "windows")]
            {
                if D3d12VideoSession::is_supported(std::ptr::null_mut()) {
                    backends.push(Backend::D3d12Video);
                }
                backends.push(Backend::MediaFoundation);
            }
            #[cfg(not(target_os = "windows"))]
            {
                // On non-Windows, D3D12/MF backends are not available.
                let _ = &mut backends;
            }
            backends
        }
        NativeHandle::D3d11 { .. } => {
            #[cfg(target_os = "windows")]
            {
                vec![Backend::MediaFoundation]
            }
            #[cfg(not(target_os = "windows"))]
            {
                vec![]
            }
        }
        NativeHandle::Vulkan { .. } => {
            let mut backends = Vec::new();
            #[cfg(target_os = "linux")]
            {
                if VkVideoSession::is_supported(std::ptr::null_mut(), std::ptr::null_mut()) {
                    backends.push(Backend::VulkanVideo);
                }
                #[cfg(feature = "gstreamer")]
                backends.push(Backend::GStreamerVaapi);
                #[cfg(feature = "v4l2")]
                if V4l2VideoSession::is_supported() {
                    backends.push(Backend::V4l2);
                }
            }
            #[cfg(target_os = "android")]
            {
                backends.push(Backend::MediaCodec);
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                // On non-Linux/Android, Vulkan Video backends are not available.
                let _ = &mut backends;
            }
            backends
        }
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
        #[cfg(target_os = "windows")]
        Backend::D3d12Video => {
            let session = D3d12VideoSession::new(path, output, config)?;
            Ok(Box::new(session))
        }
        #[cfg(target_os = "windows")]
        Backend::MediaFoundation => {
            let session = MfVideoSession::new(path, output, config)?;
            Ok(Box::new(session))
        }
        #[cfg(target_os = "linux")]
        Backend::VulkanVideo => {
            let session = VkVideoSession::new(path, output, config)?;
            Ok(Box::new(session))
        }
        #[cfg(all(target_os = "linux", feature = "gstreamer"))]
        Backend::GStreamerVaapi => {
            let session = GstVideoSession::new(path, output, config)?;
            Ok(Box::new(session))
        }
        #[cfg(all(target_os = "linux", feature = "v4l2"))]
        Backend::V4l2 => {
            let session = V4l2VideoSession::new(path, output, config)?;
            Ok(Box::new(session))
        }
        #[cfg(target_os = "android")]
        Backend::MediaCodec => {
            let session = McVideoSession::new(path, output, config)?;
            Ok(Box::new(session))
        }
        // HW backends not yet implemented (or not available on this platform).
        _ => Err(VideoError::NoHwDecoder),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::NativeHandle;

    #[test]
    fn detect_backends_d3d12_handle_on_current_platform() {
        let handle = NativeHandle::D3d12 {
            texture: std::ptr::null_mut(),
            device: std::ptr::null_mut(),
            command_queue: std::ptr::null_mut(),
        };
        let backends = detect_backends(&handle);
        // On non-Windows platforms, D3D12/MF are not available.
        #[cfg(not(target_os = "windows"))]
        assert!(
            backends.is_empty(),
            "D3D12 handle should yield no backends on non-Windows"
        );
        #[cfg(target_os = "windows")]
        assert!(
            backends.contains(&Backend::MediaFoundation),
            "D3D12 handle should include MediaFoundation on Windows"
        );
    }

    #[test]
    fn detect_backends_d3d11_handle_on_current_platform() {
        let handle = NativeHandle::D3d11 {
            texture: std::ptr::null_mut(),
            device: std::ptr::null_mut(),
        };
        let backends = detect_backends(&handle);
        #[cfg(not(target_os = "windows"))]
        assert!(
            backends.is_empty(),
            "D3D11 handle should yield no backends on non-Windows"
        );
        #[cfg(target_os = "windows")]
        assert_eq!(backends, vec![Backend::MediaFoundation]);
    }

    #[test]
    fn detect_backends_wgpu_handle() {
        let handle = NativeHandle::Wgpu {
            queue: std::ptr::null(),
            texture_id: 0,
        };
        let backends = detect_backends(&handle);
        assert_eq!(backends, vec![Backend::Software]);
    }

    #[test]
    fn backend_d3d12_and_mf_are_valid_enum_values() {
        // Verify the enum variants exist and are distinct.
        let d3d12 = Backend::D3d12Video;
        let mf = Backend::MediaFoundation;
        assert_ne!(d3d12, mf);
        assert_eq!(format!("{:?}", d3d12), "D3d12Video");
        assert_eq!(format!("{:?}", mf), "MediaFoundation");
    }

    #[test]
    fn detect_backends_vulkan_handle_on_current_platform() {
        let handle = NativeHandle::Vulkan {
            image: 0,
            device: std::ptr::null_mut(),
            physical_device: std::ptr::null_mut(),
            instance: std::ptr::null_mut(),
            queue: std::ptr::null_mut(),
            queue_family_index: 0,
        };
        let backends = detect_backends(&handle);
        // On non-Linux platforms, Vulkan Video backends are not available.
        #[cfg(not(target_os = "linux"))]
        assert!(
            backends.is_empty(),
            "Vulkan handle should yield no backends on non-Linux"
        );
        #[cfg(target_os = "linux")]
        {
            // VkVideoSession::is_supported returns false in stub, so VulkanVideo
            // should not be present. GStreamer/V4L2 depend on feature flags.
            assert!(
                !backends.contains(&Backend::VulkanVideo),
                "Stub VkVideoSession::is_supported should return false"
            );
        }
    }

    #[test]
    fn backend_linux_enum_values_are_distinct() {
        let vk = Backend::VulkanVideo;
        let gst = Backend::GStreamerVaapi;
        let v4l2 = Backend::V4l2;
        assert_ne!(vk, gst);
        assert_ne!(gst, v4l2);
        assert_ne!(vk, v4l2);
        assert_eq!(format!("{:?}", vk), "VulkanVideo");
        assert_eq!(format!("{:?}", gst), "GStreamerVaapi");
        assert_eq!(format!("{:?}", v4l2), "V4l2");
    }

    #[test]
    fn backend_media_codec_is_valid_enum_value() {
        let mc = Backend::MediaCodec;
        assert_eq!(format!("{:?}", mc), "MediaCodec");
        assert_ne!(mc, Backend::Software);
        assert_ne!(mc, Backend::VulkanVideo);
    }

    #[test]
    fn create_with_vulkan_backend_on_non_linux_returns_no_hw_decoder() {
        #[cfg(not(target_os = "linux"))]
        {
            let output = OutputTarget {
                native_handle: NativeHandle::Wgpu {
                    queue: std::ptr::null(),
                    texture_id: 0,
                },
                format: crate::types::PixelFormat::Rgba8Srgb,
                width: 640,
                height: 480,
                color_space: crate::types::ColorSpace::default(),
            };
            let config = SessionConfig::default();
            let result = create_with_backend("/dev/null", &output, &config, Backend::VulkanVideo);
            assert!(result.is_err());
        }
    }
}
