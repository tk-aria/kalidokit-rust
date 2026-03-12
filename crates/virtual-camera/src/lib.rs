/// Cross-platform virtual camera abstraction.
///
/// Platform-specific implementations:
/// - macOS: CoreMediaIO Camera Extension via `objc2-core-media-io`

pub trait VirtualCamera {
    /// Start the virtual camera device.
    fn start(&mut self) -> anyhow::Result<()>;

    /// Send a single RGBA frame to the virtual camera.
    fn send_frame(&mut self, rgba: &[u8], width: u32, height: u32) -> anyhow::Result<()>;

    /// Stop the virtual camera device.
    fn stop(&mut self);
}

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "macos")]
pub use macos::MacOsVirtualCamera;
