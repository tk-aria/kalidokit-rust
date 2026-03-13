use crate::VirtualCamera;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

extern "C" {
    /// Activate the KalidoKit Camera Extension via OSSystemExtensionManager.
    fn KalidoKitInstallCameraExtension();
}

/// TCP port for host → extension frame transfer on localhost.
/// Host runs the server, Extension connects as client.
/// Must match the Extension's StreamSource.m.
const TCP_PORT: u16 = 19876;

pub struct MacOsVirtualCamera {
    running: bool,
    frame_count: u32,
    latest_client: Arc<Mutex<Option<TcpStream>>>,
    _listener_thread: Option<std::thread::JoinHandle<()>>,
}

unsafe impl Send for MacOsVirtualCamera {}

impl Default for MacOsVirtualCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOsVirtualCamera {
    pub fn new() -> Self {
        Self {
            running: false,
            frame_count: 0,
            latest_client: Arc::new(Mutex::new(None)),
            _listener_thread: None,
        }
    }

    pub fn install_extension() {
        log::info!("[VCam] Requesting Camera Extension activation...");
        unsafe { KalidoKitInstallCameraExtension() };
    }

    fn rgba_to_bgra(data: &mut [u8]) {
        for chunk in data.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
    }
}

impl VirtualCamera for MacOsVirtualCamera {
    fn start(&mut self) -> anyhow::Result<()> {
        log::info!("[VCam] Starting macOS virtual camera (TCP server on localhost:{})", TCP_PORT);

        let listener = TcpListener::bind(("127.0.0.1", TCP_PORT))
            .map_err(|e| anyhow::anyhow!("Failed to bind TCP port {}: {}", TCP_PORT, e))?;
        listener.set_nonblocking(true).ok();

        log::info!("[VCam] TCP server listening on 127.0.0.1:{}", TCP_PORT);

        let latest_client = self.latest_client.clone();
        let handle = std::thread::spawn(move || {
            loop {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        stream.set_nonblocking(false).ok(); // listener inherits non-blocking
                        stream.set_nodelay(true).ok();
                        log::info!("[VCam] Extension connected from {}", addr);
                        // Replace previous client — only keep the latest
                        *latest_client.lock().unwrap() = Some(stream);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(_) => break,
                }
            }
        });

        self._listener_thread = Some(handle);
        self.frame_count = 0;
        self.running = true;
        Ok(())
    }

    fn send_frame(&mut self, rgba: &[u8], width: u32, height: u32) -> anyhow::Result<()> {
        if !self.running {
            anyhow::bail!("Virtual camera is not running");
        }

        // Downscale to 1280x720 to match Extension output format and reduce TCP bandwidth
        const TARGET_W: u32 = 1280;
        const TARGET_H: u32 = 720;
        let (send_data, send_w, send_h) = if width > TARGET_W || height > TARGET_H {
            let mut scaled = vec![0u8; (TARGET_W * TARGET_H * 4) as usize];
            for y in 0..TARGET_H {
                let src_y = (y as u64 * height as u64 / TARGET_H as u64) as u32;
                for x in 0..TARGET_W {
                    let src_x = (x as u64 * width as u64 / TARGET_W as u64) as u32;
                    let si = (src_y * width + src_x) as usize * 4;
                    let di = (y * TARGET_W + x) as usize * 4;
                    scaled[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
                }
            }
            Self::rgba_to_bgra(&mut scaled);
            (scaled, TARGET_W, TARGET_H)
        } else {
            let mut bgra = rgba.to_vec();
            Self::rgba_to_bgra(&mut bgra);
            (bgra, width, height)
        };

        // Frame format: [width: u32 LE][height: u32 LE][BGRA pixel data]
        let mut header = [0u8; 8];
        header[0..4].copy_from_slice(&send_w.to_le_bytes());
        header[4..8].copy_from_slice(&send_h.to_le_bytes());

        let mut guard = self.latest_client.lock().unwrap();
        if let Some(ref mut stream) = *guard {
            match stream.write_all(&header).and_then(|_| stream.write_all(&send_data)) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("[VCam] Write failed: {} (kind={:?}) frame_size={}",
                        e, e.kind(), send_data.len());
                    *guard = None;
                }
            }
        }

        self.frame_count += 1;
        if self.frame_count % 60 == 0 {
            log::info!(
                "[VCam] Frame {} sent (connected={}) ({}x{})",
                self.frame_count,
                guard.is_some(),
                send_w,
                send_h
            );
        }

        Ok(())
    }

    fn stop(&mut self) {
        if !self.running {
            return;
        }

        // Drop client connection
        *self.latest_client.lock().unwrap() = None;
        self.running = false;
        log::info!("[VCam] Virtual camera stopped");
    }
}

impl Drop for MacOsVirtualCamera {
    fn drop(&mut self) {
        self.stop();
    }
}
