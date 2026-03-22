//! winit event → imgui input mapping.
//!
//! Now a thin wrapper — actual event handling is done by ImGuiRenderer
//! via dear-imgui-winit. This module is kept for the `imgui_captured` flag.

/// Check if the dear-imgui-rs context wants to capture input.
///
/// Call this after ImGuiRenderer::handle_event has already forwarded the event.
pub fn wants_capture(io: &dear_imgui_rs::Io) -> bool {
    io.want_capture_mouse() || io.want_capture_keyboard()
}
