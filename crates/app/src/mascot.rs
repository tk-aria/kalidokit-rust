use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::window::{Window, WindowLevel};

/// State for the desktop mascot overlay mode.
pub struct MascotState {
    /// Whether mascot (transparent overlay) mode is active.
    pub enabled: bool,
    /// Whether the user is currently dragging the window.
    dragging: bool,
    /// Cursor position at drag start (physical pixels).
    drag_start_cursor: PhysicalPosition<f64>,
    /// Window position at drag start (physical pixels).
    drag_start_window: PhysicalPosition<i32>,
    /// Window size before entering mascot mode (for restoration).
    normal_size: LogicalSize<u32>,
    /// Mascot window size.
    pub mascot_size: LogicalSize<u32>,
}

impl Default for MascotState {
    fn default() -> Self {
        Self::new()
    }
}

impl MascotState {
    pub fn new() -> Self {
        Self {
            enabled: false,
            dragging: false,
            drag_start_cursor: PhysicalPosition::new(0.0, 0.0),
            drag_start_window: PhysicalPosition::new(0, 0),
            normal_size: LogicalSize::new(1280, 720),
            mascot_size: LogicalSize::new(512, 512),
        }
    }

    /// Enter mascot mode: no decorations, always on top, smaller size.
    pub fn enter(&mut self, window: &Window) {
        let size = window.inner_size();
        self.normal_size = LogicalSize::new(size.width, size.height);
        window.set_decorations(false);
        window.set_window_level(WindowLevel::AlwaysOnTop);
        let _ = window.request_inner_size(self.mascot_size);
        self.enabled = true;
        log::info!(
            "Mascot mode: ON ({}x{})",
            self.mascot_size.width,
            self.mascot_size.height
        );
    }

    /// Leave mascot mode: restore decorations, normal level, original size.
    pub fn leave(&mut self, window: &Window) {
        window.set_decorations(true);
        window.set_window_level(WindowLevel::Normal);
        let _ = window.request_inner_size(self.normal_size);
        self.enabled = false;
        self.dragging = false;
        log::info!("Mascot mode: OFF");
    }

    /// Toggle mascot mode on/off.
    pub fn toggle(&mut self, window: &Window) {
        if self.enabled {
            self.leave(window);
        } else {
            self.enter(window);
        }
    }

    /// Begin window drag on left mouse button press (only in mascot mode).
    pub fn start_drag(&mut self, window: &Window, cursor_pos: PhysicalPosition<f64>) {
        if !self.enabled {
            return;
        }
        self.dragging = true;
        self.drag_start_cursor = cursor_pos;
        self.drag_start_window = window.outer_position().unwrap_or_default();
    }

    /// Update window position during drag.
    pub fn update_drag(&self, window: &Window, cursor_pos: PhysicalPosition<f64>) {
        if !self.dragging {
            return;
        }
        let dx = cursor_pos.x - self.drag_start_cursor.x;
        let dy = cursor_pos.y - self.drag_start_cursor.y;
        let new_x = self.drag_start_window.x + dx as i32;
        let new_y = self.drag_start_window.y + dy as i32;
        window.set_outer_position(PhysicalPosition::new(new_x, new_y));
    }

    /// End window drag on mouse button release.
    pub fn end_drag(&mut self) {
        self.dragging = false;
    }

    /// Whether the user is currently dragging the mascot window.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Begin drag with explicit window position (for testing without a Window).
    #[cfg(test)]
    fn start_drag_at(&mut self, cursor: PhysicalPosition<f64>, window_pos: PhysicalPosition<i32>) {
        if !self.enabled {
            return;
        }
        self.dragging = true;
        self.drag_start_cursor = cursor;
        self.drag_start_window = window_pos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let state = MascotState::new();
        assert!(!state.enabled);
        assert!(!state.is_dragging());
        assert_eq!(state.mascot_size.width, 512);
    }

    #[test]
    fn start_drag_ignored_when_disabled() {
        let mut state = MascotState::new();
        state.start_drag_at(
            PhysicalPosition::new(100.0, 100.0),
            PhysicalPosition::new(0, 0),
        );
        assert!(!state.is_dragging());
    }

    #[test]
    fn start_drag_works_when_enabled() {
        let mut state = MascotState::new();
        state.enabled = true;
        state.start_drag_at(
            PhysicalPosition::new(100.0, 200.0),
            PhysicalPosition::new(50, 60),
        );
        assert!(state.is_dragging());
    }

    #[test]
    fn end_drag_is_noop_when_not_dragging() {
        let mut state = MascotState::new();
        state.end_drag(); // should not panic
        assert!(!state.is_dragging());
    }
}
