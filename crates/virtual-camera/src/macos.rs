use crate::VirtualCamera;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

extern "C" {
    /// Activate the KalidoKit Camera Extension via OSSystemExtensionManager.
    fn KalidoKitInstallCameraExtension();
}

/// TCP port for host → extension frame transfer on localhost.
/// Host runs the server, Extension connects as client.
/// Must match the Extension's StreamSource.m and DAL plugin.
const TCP_PORT: u16 = 19876;

/// Frame data queued for async TCP send.
struct FramePacket {
    header: [u8; 8],
    data: Vec<u8>,
}

pub struct MacOsVirtualCamera {
    running: bool,
    frame_count: u32,
    clients: Arc<Mutex<Vec<TcpStream>>>,
    _listener_thread: Option<std::thread::JoinHandle<()>>,
    /// Channel to send frames to the background TCP writer thread.
    frame_tx: Option<mpsc::SyncSender<FramePacket>>,
    _writer_thread: Option<std::thread::JoinHandle<()>>,
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
            clients: Arc::new(Mutex::new(Vec::new())),
            _listener_thread: None,
            frame_tx: None,
            _writer_thread: None,
        }
    }

    pub fn install_extension() {
        log::info!("[VCam] Requesting Camera Extension activation...");
        unsafe { KalidoKitInstallCameraExtension() };
    }
}

impl VirtualCamera for MacOsVirtualCamera {
    fn start(&mut self) -> anyhow::Result<()> {
        log::info!("[VCam] Starting macOS virtual camera (TCP server on localhost:{})", TCP_PORT);

        let listener = TcpListener::bind(("127.0.0.1", TCP_PORT))
            .map_err(|e| anyhow::anyhow!("Failed to bind TCP port {}: {}", TCP_PORT, e))?;
        listener.set_nonblocking(true).ok();

        log::info!("[VCam] TCP server listening on 127.0.0.1:{}", TCP_PORT);

        let clients = self.clients.clone();
        let handle = std::thread::spawn(move || {
            loop {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        stream.set_nonblocking(false).ok(); // listener inherits non-blocking
                        stream.set_nodelay(true).ok();
                        log::info!("[VCam] Client connected from {}", addr);
                        clients.lock().unwrap().push(stream);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(_) => break,
                }
            }
        });
        self._listener_thread = Some(handle);

        // Background TCP writer thread — bounded channel (capacity 1) drops old frames
        let (tx, rx) = mpsc::sync_channel::<FramePacket>(1);
        let clients_ref = self.clients.clone();
        let writer = std::thread::Builder::new()
            .name("vcam-tcp-writer".into())
            .spawn(move || {
                let mut sent: u32 = 0;
                while let Ok(packet) = rx.recv() {
                    let mut guard = clients_ref.lock().unwrap();
                    // Broadcast to all connected clients, remove dead ones
                    guard.retain_mut(|stream| {
                        match stream
                            .write_all(&packet.header)
                            .and_then(|_| stream.write_all(&packet.data))
                        {
                            Ok(()) => true,
                            Err(e) => {
                                log::debug!("[VCam] Client dropped: {}", e);
                                false
                            }
                        }
                    });
                    sent += 1;
                    if sent.is_multiple_of(60) {
                        log::info!(
                            "[VCam] Frame {} sent (clients={})",
                            sent,
                            guard.len()
                        );
                    }
                }
            })?;
        self._writer_thread = Some(writer);
        self.frame_tx = Some(tx);

        self.frame_count = 0;
        self.running = true;
        Ok(())
    }

    fn send_frame(&mut self, bgra: &[u8], width: u32, height: u32) -> anyhow::Result<()> {
        if !self.running {
            anyhow::bail!("Virtual camera is not running");
        }

        let mut header = [0u8; 8];
        header[0..4].copy_from_slice(&width.to_le_bytes());
        header[4..8].copy_from_slice(&height.to_le_bytes());

        let packet = FramePacket {
            header,
            data: bgra.to_vec(),
        };

        if let Some(tx) = &self.frame_tx {
            // try_send: drop frame if writer is still busy (non-blocking)
            match tx.try_send(packet) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(_)) => {
                    // Writer busy — drop this frame silently
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    log::warn!("[VCam] Writer thread disconnected");
                    self.running = false;
                }
            }
        }

        self.frame_count += 1;
        Ok(())
    }

    fn stop(&mut self) {
        if !self.running {
            return;
        }

        // Drop sender to signal writer thread to exit
        self.frame_tx.take();
        // Drop all client connections
        self.clients.lock().unwrap().clear();
        self.running = false;
        log::info!("[VCam] Virtual camera stopped");
    }
}

impl Drop for MacOsVirtualCamera {
    fn drop(&mut self) {
        self.stop();
    }
}
