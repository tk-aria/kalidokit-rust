//! ScreenCaptureKit audio capture for macOS 12.3+.
//!
//! Captures system audio output without requiring a virtual loopback device.
//! Uses raw Objective-C runtime calls with typed function pointers for ARM64 ABI correctness.

#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::resample;
use crate::{AudioError, AudioFrame, AudioSource};

// CoreMedia FFI
extern "C" {
    fn CMSampleBufferGetNumSamples(sbuf: *const c_void) -> isize;
    fn CMSampleBufferGetDataBuffer(sbuf: *const c_void) -> *const c_void;
    fn CMBlockBufferGetDataLength(block: *const c_void) -> usize;
    fn CMBlockBufferGetDataPointer(
        block: *const c_void,
        offset: usize,
        length_at_offset: *mut usize,
        total_length: *mut usize,
        data_pointer: *mut *const u8,
    ) -> i32;
}

// Objective-C runtime FFI
extern "C" {
    fn objc_getClass(name: *const i8) -> *const c_void;
    fn sel_registerName(name: *const i8) -> *const c_void;
    // On ARM64, objc_msgSend uses the callee's ABI, NOT C variadic convention.
    // We declare it with no args and transmute to typed fn pointers per call-site.
    fn objc_msgSend();
    fn objc_allocateClassPair(
        superclass: *const c_void,
        name: *const i8,
        extra_bytes: usize,
    ) -> *mut c_void;
    fn objc_registerClassPair(cls: *mut c_void);
    fn class_addMethod(
        cls: *mut c_void,
        sel: *const c_void,
        imp: *const c_void,
        types: *const i8,
    ) -> bool;
    fn class_addIvar(
        cls: *mut c_void,
        name: *const i8,
        size: usize,
        alignment: u8,
        types: *const i8,
    ) -> bool;
    fn object_getInstanceVariable(
        obj: *const c_void,
        name: *const i8,
        out_value: *mut *mut c_void,
    ) -> *const c_void;
    fn object_setInstanceVariable(
        obj: *const c_void,
        name: *const i8,
        value: *mut c_void,
    ) -> *const c_void;
    fn objc_getProtocol(name: *const i8) -> *const c_void;
    fn class_addProtocol(cls: *mut c_void, protocol: *const c_void) -> bool;
    fn dispatch_queue_create(label: *const i8, attr: *const c_void) -> *const c_void;
}

fn cls(name: &str) -> *const c_void {
    let c = std::ffi::CString::new(name).unwrap();
    unsafe { objc_getClass(c.as_ptr()) }
}

fn sel(name: &str) -> *const c_void {
    let c = std::ffi::CString::new(name).unwrap();
    unsafe { sel_registerName(c.as_ptr()) }
}

// Typed objc_msgSend wrappers — ARM64 requires exact function pointer types.
macro_rules! send {
    ($recv:expr, $sel:expr $(,)?) => {{
        let f: unsafe extern "C" fn(*const c_void, *const c_void) -> *const c_void =
            std::mem::transmute(objc_msgSend as *const ());
        f($recv, $sel)
    }};
    ($recv:expr, $sel:expr, $a:expr $(,)?) => {{
        let f: unsafe extern "C" fn(*const c_void, *const c_void, _) -> *const c_void =
            std::mem::transmute(objc_msgSend as *const ());
        f($recv, $sel, $a)
    }};
    ($recv:expr, $sel:expr, $a:expr, $b:expr $(,)?) => {{
        let f: unsafe extern "C" fn(*const c_void, *const c_void, _, _) -> *const c_void =
            std::mem::transmute(objc_msgSend as *const ());
        f($recv, $sel, $a, $b)
    }};
    ($recv:expr, $sel:expr, $a:expr, $b:expr, $c:expr $(,)?) => {{
        let f: unsafe extern "C" fn(*const c_void, *const c_void, _, _, _) -> *const c_void =
            std::mem::transmute(objc_msgSend as *const ());
        f($recv, $sel, $a, $b, $c)
    }};
    ($recv:expr, $sel:expr, $a:expr, $b:expr, $c:expr, $d:expr $(,)?) => {{
        let f: unsafe extern "C" fn(*const c_void, *const c_void, _, _, _, _) -> *const c_void =
            std::mem::transmute(objc_msgSend as *const ());
        f($recv, $sel, $a, $b, $c, $d)
    }};
}

/// Check if ScreenCaptureKit is available (macOS 12.3+).
pub fn is_available() -> bool {
    !cls("SCShareableContent").is_null()
}

/// Get macOS version as (major, minor).
pub fn macos_version() -> (u64, u64) {
    let mut size: libc::size_t = 0;
    let name = c"kern.osproductversion";
    unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        );
    }
    if size == 0 {
        return (0, 0);
    }
    let mut buf = vec![0u8; size];
    unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr() as *mut c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        );
    }
    let s = String::from_utf8_lossy(&buf);
    let s = s.trim_end_matches('\0');
    let parts: Vec<&str> = s.split('.').collect();
    let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor)
}

/// Returns true if macOS >= 14.2 (CATapDescription available).
pub fn has_catap() -> bool {
    let (major, minor) = macos_version();
    major > 14 || (major == 14 && minor >= 2)
}

/// ScreenCaptureKit-based audio capture stream.
pub struct ScAudioCapture {
    running: Arc<AtomicBool>,
    stream: *const c_void, // SCStream*
    frame_size: usize,
}

// SCStream is thread-safe (used from dispatch queues)
unsafe impl Send for ScAudioCapture {}

impl ScAudioCapture {
    pub fn new(frame_size: usize) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            stream: std::ptr::null(),
            frame_size,
        }
    }

    /// Start capturing system audio via ScreenCaptureKit.
    pub fn start<F>(&mut self, callback: F) -> Result<(), AudioError>
    where
        F: FnMut(AudioFrame) + Send + 'static,
    {
        if !is_available() {
            return Err(AudioError::StreamError(
                "ScreenCaptureKit not available (requires macOS 12.3+)".to_string(),
            ));
        }

        self.running.store(true, Ordering::Relaxed);
        let frame_size = self.frame_size;

        // 1. Create SCStreamConfiguration
        let config = unsafe {
            let c: *const c_void = send!(cls("SCStreamConfiguration"), sel("new"));
            // setCapturesAudio: YES (BOOL = bool on ARM64)
            send!(c, sel("setCapturesAudio:"), true);
            // setExcludesCurrentProcessAudio: NO
            send!(c, sel("setExcludesCurrentProcessAudio:"), false);
            // Minimize video: 1x1
            send!(c, sel("setWidth:"), 1usize);
            send!(c, sel("setHeight:"), 1usize);
            // Audio: 16kHz mono
            send!(c, sel("setSampleRate:"), 16000i64);
            send!(c, sel("setChannelCount:"), 1i64);
            c
        };

        // 2. Get shareable content synchronously
        let content = Self::get_shareable_content()?;

        // 3. Get first display
        let displays: *const c_void = unsafe { send!(content, sel("displays")) };
        let count: usize =
            unsafe { std::mem::transmute(send!(displays, sel("count"))) };
        if count == 0 {
            return Err(AudioError::StreamError(
                "No displays found for ScreenCaptureKit".to_string(),
            ));
        }
        let display: *const c_void =
            unsafe { send!(displays, sel("objectAtIndex:"), 0usize) };

        // 4. Create content filter
        let empty: *const c_void = unsafe { send!(cls("NSArray"), sel("array")) };
        let filter: *const c_void = unsafe {
            let alloc = send!(cls("SCContentFilter"), sel("alloc"));
            send!(
                alloc,
                sel("initWithDisplay:excludingApplications:exceptingWindows:"),
                display,
                empty,
                empty,
            )
        };

        // 5. Create SCStream
        let stream: *const c_void = unsafe {
            let alloc = send!(cls("SCStream"), sel("alloc"));
            send!(
                alloc,
                sel("initWithFilter:configuration:delegate:"),
                filter,
                config,
                std::ptr::null::<c_void>(),
            )
        };

        // 6. Create dispatch queue
        let queue_label = std::ffi::CString::new("audio-capture.sc-audio").unwrap();
        let queue = unsafe { dispatch_queue_create(queue_label.as_ptr(), std::ptr::null()) };

        // 7. Create SCStreamOutput delegate with runtime-defined ObjC class
        let delegate = Self::create_output_delegate(callback, frame_size, self.running.clone())?;

        // 8. Add output delegate: addStreamOutput:type:sampleHandlerQueue:error:
        // SCStreamOutputType.audio = 1
        unsafe {
            let mut err: *const c_void = std::ptr::null();
            send!(
                stream,
                sel("addStreamOutput:type:sampleHandlerQueue:error:"),
                delegate,
                1isize, // SCStreamOutputType.audio
                queue,
                &mut err as *mut *const c_void,
            );
            if !err.is_null() {
                return Err(AudioError::StreamError(
                    "Failed to add SCStream output delegate".to_string(),
                ));
            }
        }

        self.stream = stream;

        // 9. Start capture
        let (start_tx, start_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let start_block = block2::RcBlock::new(move |error: *const c_void| {
            if error.is_null() {
                let _ = start_tx.send(Ok(()));
            } else {
                let _ = start_tx.send(Err("SCStream start failed".to_string()));
            }
        });

        unsafe {
            send!(
                stream,
                sel("startCaptureWithCompletionHandler:"),
                &*start_block as *const _ as *const c_void,
            );
        }

        match start_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => {
                log::info!("ScreenCaptureKit audio capture started");
                Ok(())
            }
            Ok(Err(e)) => Err(AudioError::StreamError(e)),
            Err(_) => Err(AudioError::StreamError(
                "SCStream start timed out".to_string(),
            )),
        }
    }

    /// Create a runtime ObjC class implementing SCStreamOutput protocol.
    fn create_output_delegate<F>(
        callback: F,
        frame_size: usize,
        running: Arc<AtomicBool>,
    ) -> Result<*const c_void, AudioError>
    where
        F: FnMut(AudioFrame) + Send + 'static,
    {
        use std::sync::Once;
        static REGISTER: Once = Once::new();
        static mut DELEGATE_CLASS: *const c_void = std::ptr::null();

        REGISTER.call_once(|| unsafe {
            let superclass = cls("NSObject");
            let class_name = c"RustSCStreamOutputDelegate";
            let new_class = objc_allocateClassPair(superclass, class_name.as_ptr(), 0);
            assert!(!new_class.is_null(), "Failed to create ObjC delegate class");

            // Add ivar for the Rust context pointer
            class_addIvar(
                new_class,
                c"rustCtx".as_ptr(),
                std::mem::size_of::<*mut c_void>(),
                std::mem::align_of::<*mut c_void>() as u8,
                c"^v".as_ptr(),
            );

            // Add SCStreamOutput protocol
            let protocol = objc_getProtocol(c"SCStreamOutput".as_ptr());
            if !protocol.is_null() {
                class_addProtocol(new_class, protocol);
            }

            // stream:didOutputSampleBuffer:ofType:
            // type encoding: v@:@@q  (void, id, SEL, id, id, NSInteger)
            class_addMethod(
                new_class,
                sel("stream:didOutputSampleBuffer:ofType:"),
                handle_sample_buffer as *const c_void,
                c"v@:@@q".as_ptr(),
            );

            objc_registerClassPair(new_class);
            DELEGATE_CLASS = new_class as *const c_void;
        });

        let ctx = Box::new(DelegateContext {
            callback: Box::new(callback),
            buffer: Vec::new(),
            frame_size,
            running,
            start_time: Instant::now(),
        });
        let ctx_ptr = Box::into_raw(ctx) as *mut c_void;

        let delegate = unsafe {
            let obj: *const c_void = send!(DELEGATE_CLASS, sel("new"));
            if obj.is_null() {
                drop(Box::from_raw(ctx_ptr as *mut DelegateContext));
                return Err(AudioError::StreamError(
                    "Failed to allocate SCStreamOutput delegate".to_string(),
                ));
            }
            object_setInstanceVariable(obj, c"rustCtx".as_ptr(), ctx_ptr);
            obj
        };

        Ok(delegate)
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if !self.stream.is_null() {
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            let stop_block = block2::RcBlock::new(move |_error: *const c_void| {
                let _ = tx.send(());
            });
            unsafe {
                send!(
                    self.stream,
                    sel("stopCaptureWithCompletionHandler:"),
                    &*stop_block as *const _ as *const c_void,
                );
            }
            let _ = rx.recv_timeout(Duration::from_secs(5));
            self.stream = std::ptr::null();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Synchronously fetch SCShareableContent.
    fn get_shareable_content() -> Result<*const c_void, AudioError> {
        let (tx, rx) = std::sync::mpsc::channel();

        let completion = block2::RcBlock::new(
            move |content: *const c_void, error: *const c_void| {
                if !content.is_null() {
                    let retained = unsafe { send!(content, sel("retain")) };
                    let _ = tx.send(Ok(retained));
                } else if !error.is_null() {
                    let _ = tx.send(Err("SCShareableContent error".to_string()));
                } else {
                    let _ = tx.send(Err("unknown error".to_string()));
                }
            },
        );

        let block_ptr = &*completion as *const _ as *const c_void;

        unsafe {
            send!(
                cls("SCShareableContent"),
                sel("getShareableContentExcludingDesktopWindows:onScreenWindowsOnly:completionHandler:"),
                true,  // BOOL excludeDesktopWindows
                true,  // BOOL onScreenWindowsOnly
                block_ptr,
            );
        }

        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(content)) => Ok(content),
            Ok(Err(e)) => Err(AudioError::StreamError(format!(
                "{e} (screen recording permission may be required)"
            ))),
            Err(_) => Err(AudioError::StreamError(
                "SCShareableContent timed out (grant screen recording permission)".to_string(),
            )),
        }
    }

    /// Extract f32 audio samples from a CMSampleBuffer pointer.
    pub unsafe fn extract_audio_samples(sample_buffer: *const c_void) -> Vec<f32> {
        let num_samples = CMSampleBufferGetNumSamples(sample_buffer);
        if num_samples <= 0 {
            return Vec::new();
        }

        let block_buf = CMSampleBufferGetDataBuffer(sample_buffer);
        if block_buf.is_null() {
            return Vec::new();
        }

        let data_len = CMBlockBufferGetDataLength(block_buf);
        if data_len == 0 {
            return Vec::new();
        }

        let mut data_ptr: *const u8 = std::ptr::null();
        let mut length_at_offset: usize = 0;
        let mut total_length: usize = 0;
        let status = CMBlockBufferGetDataPointer(
            block_buf,
            0,
            &mut length_at_offset,
            &mut total_length,
            &mut data_ptr,
        );

        if status != 0 || data_ptr.is_null() {
            return Vec::new();
        }

        let float_count = total_length / 4;
        let float_slice = std::slice::from_raw_parts(data_ptr as *const f32, float_count);
        float_slice.to_vec()
    }
}

/// Context stored in the ObjC delegate's ivar.
struct DelegateContext {
    callback: Box<dyn FnMut(AudioFrame) + Send>,
    buffer: Vec<i16>,
    frame_size: usize,
    running: Arc<AtomicBool>,
    start_time: Instant,
}

/// C function called by the ObjC runtime for `stream:didOutputSampleBuffer:ofType:`.
extern "C" fn handle_sample_buffer(
    this: *const c_void,
    _cmd: *const c_void,
    _stream: *const c_void,
    sample_buffer: *const c_void,
    output_type: isize,
) {
    // SCStreamOutputType.audio = 1
    if output_type != 1 || sample_buffer.is_null() {
        return;
    }

    unsafe {
        let mut ctx_ptr: *mut c_void = std::ptr::null_mut();
        object_getInstanceVariable(this, c"rustCtx".as_ptr(), &mut ctx_ptr);
        if ctx_ptr.is_null() {
            return;
        }

        let ctx = &mut *(ctx_ptr as *mut DelegateContext);
        if !ctx.running.load(Ordering::Relaxed) {
            return;
        }

        let samples_f32 = ScAudioCapture::extract_audio_samples(sample_buffer);
        if samples_f32.is_empty() {
            return;
        }

        // SCStreamConfiguration already set to 16kHz mono — convert f32 → i16.
        let pcm = resample::f32_to_i16(&samples_f32);
        ctx.buffer.extend_from_slice(&pcm);

        while ctx.buffer.len() >= ctx.frame_size {
            let frame_samples: Vec<i16> = ctx.buffer.drain(..ctx.frame_size).collect();
            (ctx.callback)(AudioFrame {
                samples: frame_samples,
                sample_rate: 16000,
                timestamp: ctx.start_time.elapsed(),
                source: AudioSource::Output,
            });
        }
    }
}

impl Drop for ScAudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
