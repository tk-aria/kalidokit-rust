use crate::VirtualCamera;
use std::ptr::{self, NonNull};

use objc2_core_media::CMTime;
use objc2_core_media_io::{
    CMIODeviceStartStream, CMIODeviceStopStream, CMIOObjectGetPropertyData,
    CMIOObjectGetPropertyDataSize, CMIOObjectPropertyAddress, CMIOStreamCopyBufferQueue,
};
use objc2_core_video::CVPixelBuffer;

/// Pixel format: 32-bit BGRA
const K_CV_PIXEL_FORMAT_TYPE_32_BGRA: u32 = 0x42475241;

/// CoreMediaIO system object ID
const K_CMIO_OBJECT_SYSTEM_OBJECT: u32 = 1;

/// Property selectors
const K_CMIO_HARDWARE_PROPERTY_DEVICES: u32 = 0x64657623; // 'dev#'
const K_CMIO_DEVICE_PROPERTY_STREAMS: u32 = 0x73746D23; // 'stm#'
const K_CMIO_OBJECT_PROPERTY_NAME: u32 = 0x6C6E616D; // 'lnam'
const K_CMIO_OBJECT_PROPERTY_SCOPE_WILDCARD: u32 = 0x2A2A2A2A;
const K_CMIO_OBJECT_PROPERTY_ELEMENT_WILDCARD: u32 = 0xFFFFFFFF;
const K_CMIO_STREAM_PROPERTY_DIRECTION: u32 = 0x73646972; // 'sdir'

pub struct MacOsVirtualCamera {
    running: bool,
    device_id: Option<u32>,
    sink_stream_id: Option<u32>,
    buffer_queue: Option<NonNull<objc2_core_media::CMSimpleQueue>>,
    frame_count: i64,
    width: u32,
    height: u32,
}

impl MacOsVirtualCamera {
    pub fn new() -> Self {
        Self {
            running: false,
            device_id: None,
            sink_stream_id: None,
            buffer_queue: None,
            frame_count: 0,
            width: 1280,
            height: 720,
        }
    }

    /// Find the KalidoKit virtual camera device and its sink stream.
    fn discover_device(&mut self) -> anyhow::Result<()> {
        let devices = get_cmio_devices()?;
        log::info!("[VCam] Found {} CoreMediaIO devices", devices.len());

        for device_id in &devices {
            let name = get_object_name(*device_id);
            log::info!("[VCam] Device {}: {:?}", device_id, name);

            if let Some(ref name) = name {
                if name.contains("KalidoKit") {
                    self.device_id = Some(*device_id);

                    // Find sink stream (direction = 1 = sink)
                    let streams = get_device_streams(*device_id)?;
                    for stream_id in &streams {
                        let direction = get_stream_direction(*stream_id);
                        log::info!("[VCam]   Stream {}: direction={}", stream_id, direction);
                        if direction == 1 {
                            // Sink direction
                            self.sink_stream_id = Some(*stream_id);
                            break;
                        }
                    }
                    break;
                }
            }
        }

        if self.device_id.is_none() {
            anyhow::bail!("KalidoKit virtual camera device not found. Is the Camera Extension installed?");
        }
        if self.sink_stream_id.is_none() {
            anyhow::bail!("KalidoKit sink stream not found on device");
        }

        Ok(())
    }

    /// Get the buffer queue for the sink stream.
    fn acquire_buffer_queue(&mut self) -> anyhow::Result<()> {
        let stream_id = self.sink_stream_id
            .ok_or_else(|| anyhow::anyhow!("No sink stream"))?;

        let mut queue: *mut objc2_core_media::CMSimpleQueue = ptr::null_mut();
        let status = unsafe {
            CMIOStreamCopyBufferQueue(
                stream_id,
                None,
                ptr::null_mut(),
                &mut queue,
            )
        };
        if status != 0 {
            anyhow::bail!("CMIOStreamCopyBufferQueue failed: {}", status);
        }
        if queue.is_null() {
            anyhow::bail!("CMIOStreamCopyBufferQueue returned null queue");
        }
        self.buffer_queue = NonNull::new(queue);
        Ok(())
    }

    /// Convert RGBA bytes to BGRA in-place.
    fn rgba_to_bgra(data: &mut [u8]) {
        for chunk in data.chunks_exact_mut(4) {
            chunk.swap(0, 2); // R <-> B
        }
    }

    /// Create a CVPixelBuffer from BGRA data.
    fn create_pixel_buffer(&self, bgra: &mut [u8]) -> anyhow::Result<NonNull<CVPixelBuffer>> {
        let mut pixel_buffer: *mut CVPixelBuffer = ptr::null_mut();
        let bytes_per_row = self.width as usize * 4;

        let status = unsafe {
            objc2_core_video::CVPixelBufferCreateWithBytes(
                None,
                self.width as usize,
                self.height as usize,
                K_CV_PIXEL_FORMAT_TYPE_32_BGRA,
                NonNull::new(bgra.as_mut_ptr().cast()).unwrap(),
                bytes_per_row,
                None, // release callback (not needed, we own the data)
                ptr::null_mut(),
                None,
                NonNull::new(&mut pixel_buffer).unwrap(),
            )
        };
        if status != 0 {
            anyhow::bail!("CVPixelBufferCreateWithBytes failed: {}", status);
        }
        NonNull::new(pixel_buffer)
            .ok_or_else(|| anyhow::anyhow!("CVPixelBufferCreateWithBytes returned null"))
    }

    /// Create a CMSampleBuffer from a CVPixelBuffer.
    fn create_sample_buffer(
        &self,
        pixel_buffer: NonNull<CVPixelBuffer>,
    ) -> anyhow::Result<NonNull<objc2_core_media::CMSampleBuffer>> {
        // Create format description from pixel buffer
        let mut format_desc: *const objc2_core_media::CMVideoFormatDescription = ptr::null();
        let status = unsafe {
            objc2_core_media::CMVideoFormatDescriptionCreateForImageBuffer(
                None,
                &*pixel_buffer.as_ptr(),
                NonNull::new(&mut format_desc as *mut _ as *mut _).unwrap(),
            )
        };
        if status != 0 {
            anyhow::bail!("CMVideoFormatDescriptionCreateForImageBuffer failed: {}", status);
        }

        // Timing info
        let pts = unsafe { CMTime::new(self.frame_count, 30) };
        let duration = unsafe { CMTime::new(1, 30) };
        let mut timing = objc2_core_media::CMSampleTimingInfo {
            duration,
            presentationTimeStamp: pts,
            decodeTimeStamp: unsafe { *(&raw const objc2_core_media::kCMTimeInvalid) },
        };

        // Create sample buffer
        let mut sample_buffer: *mut objc2_core_media::CMSampleBuffer = ptr::null_mut();
        let status = unsafe {
            objc2_core_media::CMSampleBuffer::create_ready_with_image_buffer(
                None,
                &*pixel_buffer.as_ptr(),
                &*format_desc,
                NonNull::new(&mut timing).unwrap(),
                NonNull::new(&mut sample_buffer).unwrap(),
            )
        };
        if status != 0 {
            anyhow::bail!("CMSampleBufferCreateReadyWithImageBuffer failed: {}", status);
        }

        NonNull::new(sample_buffer)
            .ok_or_else(|| anyhow::anyhow!("CMSampleBufferCreateReadyWithImageBuffer returned null"))
    }
}

impl VirtualCamera for MacOsVirtualCamera {
    fn start(&mut self) -> anyhow::Result<()> {
        log::info!("[VCam] Starting macOS virtual camera");

        self.discover_device()?;
        self.acquire_buffer_queue()?;

        let device_id = self.device_id.unwrap();
        let stream_id = self.sink_stream_id.unwrap();

        let status = unsafe { CMIODeviceStartStream(device_id, stream_id) };
        if status != 0 {
            anyhow::bail!("CMIODeviceStartStream failed: {}", status);
        }

        self.running = true;
        log::info!("[VCam] Virtual camera started (device={}, stream={})", device_id, stream_id);
        Ok(())
    }

    fn send_frame(&mut self, rgba: &[u8], width: u32, height: u32) -> anyhow::Result<()> {
        if !self.running {
            anyhow::bail!("Virtual camera is not running");
        }

        self.width = width;
        self.height = height;

        // Convert RGBA → BGRA
        let mut bgra = rgba.to_vec();
        Self::rgba_to_bgra(&mut bgra);

        // Create CVPixelBuffer → CMSampleBuffer
        let pixel_buffer = self.create_pixel_buffer(&mut bgra)?;
        let sample_buffer = self.create_sample_buffer(pixel_buffer)?;

        // Enqueue into buffer queue
        if let Some(queue) = self.buffer_queue {
            let status = unsafe {
                (*queue.as_ptr()).enqueue(sample_buffer.cast())
            };
            if status != 0 {
                anyhow::bail!("CMSimpleQueue enqueue failed: {}", status);
            }
        }

        self.frame_count += 1;
        Ok(())
    }

    fn stop(&mut self) {
        if !self.running {
            return;
        }

        if let (Some(device_id), Some(stream_id)) = (self.device_id, self.sink_stream_id) {
            unsafe {
                CMIODeviceStopStream(device_id, stream_id);
            }
        }

        self.running = false;
        self.buffer_queue = None;
        log::info!("[VCam] Virtual camera stopped");
    }
}

// --- Helper functions for CoreMediaIO property queries ---

fn make_address(selector: u32) -> CMIOObjectPropertyAddress {
    CMIOObjectPropertyAddress {
        mSelector: selector,
        mScope: K_CMIO_OBJECT_PROPERTY_SCOPE_WILDCARD,
        mElement: K_CMIO_OBJECT_PROPERTY_ELEMENT_WILDCARD,
    }
}

fn get_cmio_devices() -> anyhow::Result<Vec<u32>> {
    let address = make_address(K_CMIO_HARDWARE_PROPERTY_DEVICES);
    let mut data_size: u32 = 0;

    let status = unsafe {
        CMIOObjectGetPropertyDataSize(
            K_CMIO_OBJECT_SYSTEM_OBJECT,
            &address,
            0,
            ptr::null(),
            &mut data_size,
        )
    };
    if status != 0 {
        anyhow::bail!("CMIOObjectGetPropertyDataSize (devices) failed: {}", status);
    }

    let count = data_size as usize / std::mem::size_of::<u32>();
    let mut devices = vec![0u32; count];
    let mut data_used: u32 = 0;

    let status = unsafe {
        CMIOObjectGetPropertyData(
            K_CMIO_OBJECT_SYSTEM_OBJECT,
            &address,
            0,
            ptr::null(),
            data_size,
            &mut data_used,
            devices.as_mut_ptr().cast(),
        )
    };
    if status != 0 {
        anyhow::bail!("CMIOObjectGetPropertyData (devices) failed: {}", status);
    }

    Ok(devices)
}

fn get_device_streams(device_id: u32) -> anyhow::Result<Vec<u32>> {
    let address = make_address(K_CMIO_DEVICE_PROPERTY_STREAMS);
    let mut data_size: u32 = 0;

    let status = unsafe {
        CMIOObjectGetPropertyDataSize(device_id, &address, 0, ptr::null(), &mut data_size)
    };
    if status != 0 {
        anyhow::bail!("CMIOObjectGetPropertyDataSize (streams) failed: {}", status);
    }

    let count = data_size as usize / std::mem::size_of::<u32>();
    let mut streams = vec![0u32; count];
    let mut data_used: u32 = 0;

    let status = unsafe {
        CMIOObjectGetPropertyData(
            device_id,
            &address,
            0,
            ptr::null(),
            data_size,
            &mut data_used,
            streams.as_mut_ptr().cast(),
        )
    };
    if status != 0 {
        anyhow::bail!("CMIOObjectGetPropertyData (streams) failed: {}", status);
    }

    Ok(streams)
}

fn get_object_name(object_id: u32) -> Option<String> {
    let address = make_address(K_CMIO_OBJECT_PROPERTY_NAME);
    let mut data_size: u32 = 0;

    let status = unsafe {
        CMIOObjectGetPropertyDataSize(object_id, &address, 0, ptr::null(), &mut data_size)
    };
    if status != 0 {
        return None;
    }

    // Name is a CFStringRef
    let mut cf_str: *const std::ffi::c_void = ptr::null();
    let mut data_used: u32 = 0;

    let status = unsafe {
        CMIOObjectGetPropertyData(
            object_id,
            &address,
            0,
            ptr::null(),
            data_size,
            &mut data_used,
            (&mut cf_str as *mut *const std::ffi::c_void).cast(),
        )
    };
    if status != 0 || cf_str.is_null() {
        return None;
    }

    // Convert CFStringRef to Rust String using CoreFoundation
    unsafe {
        let cf_str = cf_str as *const objc2_core_foundation::CFString;
        let len = objc2_core_foundation::CFString::length(&*cf_str);
        let mut buf = vec![0u8; (len as usize) * 4 + 1];
        let range = objc2_core_foundation::CFRange { location: 0, length: len };
        let mut used: isize = 0;
        let k_cf_string_encoding_utf8: u32 = 0x08000100;
        objc2_core_foundation::CFString::bytes(
            &*cf_str,
            range,
            k_cf_string_encoding_utf8,
            0,
            false,
            buf.as_mut_ptr(),
            buf.len() as isize,
            &mut used,
        );
        Some(String::from_utf8_lossy(&buf[..used as usize]).into_owned())
    }
}

fn get_stream_direction(stream_id: u32) -> u32 {
    let address = make_address(K_CMIO_STREAM_PROPERTY_DIRECTION);
    let mut direction: u32 = 0;
    let mut data_used: u32 = 0;

    let status = unsafe {
        CMIOObjectGetPropertyData(
            stream_id,
            &address,
            0,
            ptr::null(),
            std::mem::size_of::<u32>() as u32,
            &mut data_used,
            (&mut direction as *mut u32).cast(),
        )
    };
    if status != 0 {
        return u32::MAX;
    }
    direction
}
