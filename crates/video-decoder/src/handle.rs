use std::ffi::c_void;

/// GPU native texture handle provided by the application.
///
/// # Safety
/// Pointers must remain valid for the lifetime of the VideoSession.
#[derive(Debug, Clone, Copy)]
pub enum NativeHandle {
    /// macOS / iOS: `id<MTLTexture>` + `id<MTLDevice>`
    Metal {
        texture: *mut c_void,
        device: *mut c_void,
    },

    /// Windows (D3D12 Video — primary): `ID3D12Resource*`
    D3d12 {
        texture: *mut c_void,
        device: *mut c_void,
        command_queue: *mut c_void,
    },

    /// Windows (Media Foundation fallback): `ID3D11Texture2D*`
    D3d11 {
        texture: *mut c_void,
        device: *mut c_void,
    },

    /// Linux / Android (Vulkan backend)
    Vulkan {
        image: u64,
        device: *mut c_void,
        physical_device: *mut c_void,
        instance: *mut c_void,
        queue: *mut c_void,
        queue_family_index: u32,
    },

    /// CPU fallback: wgpu Queue + Texture ID
    Wgpu {
        queue: *const c_void,
        texture_id: u64,
    },
}

// Safety: Native handles reference GPU resources which are thread-safe
// by their respective API guarantees.
unsafe impl Send for NativeHandle {}
unsafe impl Sync for NativeHandle {}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}

    #[test]
    fn native_handle_is_send_sync() {
        _assert_send::<NativeHandle>();
        _assert_sync::<NativeHandle>();
    }

    #[test]
    fn wgpu_handle_create_and_clone() {
        let h = NativeHandle::Wgpu {
            queue: std::ptr::null(),
            texture_id: 42,
        };
        let h2 = h;
        match h2 {
            NativeHandle::Wgpu { texture_id, .. } => assert_eq!(texture_id, 42),
            _ => panic!("expected Wgpu variant"),
        }
    }

    #[test]
    fn d3d12_handle_create() {
        let h = NativeHandle::D3d12 {
            texture: std::ptr::null_mut(),
            device: std::ptr::null_mut(),
            command_queue: std::ptr::null_mut(),
        };
        assert!(matches!(h, NativeHandle::D3d12 { .. }));
    }
}
