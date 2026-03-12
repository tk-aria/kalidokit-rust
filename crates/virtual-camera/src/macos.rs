use crate::VirtualCamera;

pub struct MacOsVirtualCamera {
    running: bool,
}

impl MacOsVirtualCamera {
    pub fn new() -> Self {
        Self { running: false }
    }
}

impl VirtualCamera for MacOsVirtualCamera {
    fn start(&mut self) -> anyhow::Result<()> {
        log::info!("macOS virtual camera starting");
        self.running = true;
        Ok(())
    }

    fn send_frame(&mut self, _rgba: &[u8], _width: u32, _height: u32) -> anyhow::Result<()> {
        if !self.running {
            anyhow::bail!("Virtual camera is not running");
        }
        Ok(())
    }

    fn stop(&mut self) {
        log::info!("macOS virtual camera stopping");
        self.running = false;
    }
}
