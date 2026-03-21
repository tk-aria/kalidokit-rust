//! winit event → imgui input mapping.
//!
//! Translates winit 0.30 WindowEvent into imgui IO state updates.

use imgui::Context;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::{Key, NamedKey};

/// Update imgui IO from a winit event.
/// Returns true if imgui wants to capture this event (mouse over imgui window, etc.).
pub fn handle_event(imgui: &mut Context, event: &WindowEvent) -> bool {
    let io = imgui.io_mut();

    match event {
        WindowEvent::CursorMoved { position, .. } => {
            let scale = io.display_framebuffer_scale[0];
            io.add_mouse_pos_event([
                position.x as f32 / scale,
                position.y as f32 / scale,
            ]);
            io.want_capture_mouse
        }
        WindowEvent::MouseInput { state, button, .. } => {
            let btn = match button {
                MouseButton::Left => imgui::MouseButton::Left,
                MouseButton::Right => imgui::MouseButton::Right,
                MouseButton::Middle => imgui::MouseButton::Middle,
                _ => return false,
            };
            io.add_mouse_button_event(btn, *state == ElementState::Pressed);
            io.want_capture_mouse
        }
        WindowEvent::MouseWheel { delta, .. } => {
            let (h, v) = match delta {
                winit::event::MouseScrollDelta::LineDelta(h, v) => (*h, *v),
                winit::event::MouseScrollDelta::PixelDelta(pos) => {
                    (pos.x as f32 / 20.0, pos.y as f32 / 20.0)
                }
            };
            io.add_mouse_wheel_event([h, v]);
            io.want_capture_mouse
        }
        WindowEvent::KeyboardInput { event, .. } => {
            let pressed = event.state == ElementState::Pressed;

            // Map winit key to imgui key
            if let Some(imgui_key) = map_key(&event.logical_key) {
                io.add_key_event(imgui_key, pressed);
            }

            // Text input
            if pressed {
                if let Key::Character(ch) = &event.logical_key {
                    for c in ch.chars() {
                        if !c.is_control() {
                            io.add_input_character(c);
                        }
                    }
                }
            }

            io.want_capture_keyboard
        }
        WindowEvent::Focused(focused) => {
            io.app_focus_lost = !focused;
            false
        }
        _ => false,
    }
}

/// Map winit logical key to imgui key.
fn map_key(key: &Key) -> Option<imgui::Key> {
    Some(match key {
        Key::Named(NamedKey::Tab) => imgui::Key::Tab,
        Key::Named(NamedKey::ArrowLeft) => imgui::Key::LeftArrow,
        Key::Named(NamedKey::ArrowRight) => imgui::Key::RightArrow,
        Key::Named(NamedKey::ArrowUp) => imgui::Key::UpArrow,
        Key::Named(NamedKey::ArrowDown) => imgui::Key::DownArrow,
        Key::Named(NamedKey::PageUp) => imgui::Key::PageUp,
        Key::Named(NamedKey::PageDown) => imgui::Key::PageDown,
        Key::Named(NamedKey::Home) => imgui::Key::Home,
        Key::Named(NamedKey::End) => imgui::Key::End,
        Key::Named(NamedKey::Insert) => imgui::Key::Insert,
        Key::Named(NamedKey::Delete) => imgui::Key::Delete,
        Key::Named(NamedKey::Backspace) => imgui::Key::Backspace,
        Key::Named(NamedKey::Space) => imgui::Key::Space,
        Key::Named(NamedKey::Enter) => imgui::Key::Enter,
        Key::Named(NamedKey::Escape) => imgui::Key::Escape,
        Key::Named(NamedKey::Control) => imgui::Key::LeftCtrl,
        Key::Named(NamedKey::Shift) => imgui::Key::LeftShift,
        Key::Named(NamedKey::Alt) => imgui::Key::LeftAlt,
        Key::Named(NamedKey::Super) => imgui::Key::LeftSuper,
        Key::Character(c) => match c.as_ref() {
            "a" => imgui::Key::A,
            "b" => imgui::Key::B,
            "c" => imgui::Key::C,
            "d" => imgui::Key::D,
            "e" => imgui::Key::E,
            "f" => imgui::Key::F,
            "v" => imgui::Key::V,
            "x" => imgui::Key::X,
            "y" => imgui::Key::Y,
            "z" => imgui::Key::Z,
            _ => return None,
        },
        _ => return None,
    })
}

