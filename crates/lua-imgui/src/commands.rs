//! ImGui command buffer — collected from Lua, replayed during imgui frame.

use dear_imgui_rs::Ui;

/// A single imgui command captured from Lua.
#[derive(Debug, Clone)]
pub enum ImguiCommand {
    BeginWindow {
        name: String,
    },
    EndWindow,
    Text {
        text: String,
    },
    Button {
        label: String,
    },
    SameLine,
    Separator,
    SliderFloat {
        label: String,
        min: f32,
        max: f32,
        value: f32,
    },
    Checkbox {
        label: String,
        checked: bool,
    },
    InputText {
        label: String,
        text: String,
    },
    TreeNodeBegin {
        label: String,
    },
    TreeNodeEnd,
    ColorEdit3 {
        label: String,
        color: [f32; 3],
    },
    CollapsingHeaderBegin {
        label: String,
        default_open: bool,
    },
    CollapsingHeaderEnd,
    TextDisabled {
        text: String,
    },
    Indent,
    Unindent,
}

/// Replay collected commands against an active imgui frame.
pub fn replay(commands: &[ImguiCommand], ui: &Ui) {
    let mut in_window = false;
    let mut in_tree = false;

    for cmd in commands {
        match cmd {
            ImguiCommand::BeginWindow { name } => {
                ui.window(name).build(|| {
                    // Window content will be filled by subsequent commands
                    // This is a simplification — actual nesting is handled
                    // by the begin/end pairing below.
                });
                // For proper begin/end pairing, we need the token approach:
                // Actually, imgui-rs requires closures for windows.
                // We handle this differently — see replay_nested below.
                in_window = true;
            }
            ImguiCommand::EndWindow => {
                in_window = false;
            }
            ImguiCommand::Text { text } => {
                ui.text(text);
            }
            ImguiCommand::Button { label } => {
                ui.button(label);
            }
            ImguiCommand::SameLine => {
                ui.same_line();
            }
            ImguiCommand::Separator => {
                ui.separator();
            }
            ImguiCommand::SliderFloat {
                label,
                min,
                max,
                value,
            } => {
                let mut v = *value;
                ui.slider(label, *min, *max, &mut v);
            }
            ImguiCommand::Checkbox { label, checked } => {
                let mut c = *checked;
                ui.checkbox(label, &mut c);
            }
            ImguiCommand::InputText { label: _, text: _ } => {
                // TODO: requires mutable string buffer
            }
            ImguiCommand::TreeNodeBegin { label } => {
                in_tree = ui.tree_node(label).is_some();
            }
            ImguiCommand::TreeNodeEnd => {
                in_tree = false;
            }
            ImguiCommand::ColorEdit3 { label, color } => {
                let mut c = *color;
                ui.color_edit3(label, &mut c);
            }
            ImguiCommand::CollapsingHeaderBegin { .. }
            | ImguiCommand::CollapsingHeaderEnd
            | ImguiCommand::TextDisabled { .. }
            | ImguiCommand::Indent
            | ImguiCommand::Unindent => {}
        }
    }
    let _ = (in_window, in_tree);
}

/// Replay commands with proper window nesting.
///
/// This groups commands between BeginWindow/EndWindow and executes them
/// inside `ui.window(...).build(|| { ... })`.
pub fn replay_nested(commands: &[ImguiCommand], ui: &Ui) {
    let mut i = 0;
    while i < commands.len() {
        match &commands[i] {
            ImguiCommand::BeginWindow { name } => {
                let name = name.clone();
                // Find matching EndWindow
                let start = i + 1;
                let mut depth = 1;
                let mut end = start;
                while end < commands.len() {
                    match &commands[end] {
                        ImguiCommand::BeginWindow { .. } => depth += 1,
                        ImguiCommand::EndWindow if depth == 1 => break,
                        ImguiCommand::EndWindow => depth -= 1,
                        _ => {}
                    }
                    end += 1;
                }
                let inner = &commands[start..end];
                let empty_outputs = std::sync::Arc::new(std::sync::Mutex::new(
                    std::collections::HashMap::new(),
                ));
                ui.window(&name).build(|| {
                    replay_inner(inner, ui, &empty_outputs);
                });
                i = end + 1; // skip past EndWindow
            }
            _ => {
                // Top-level command outside any window — skip
                i += 1;
            }
        }
    }
}

pub fn replay_inner(commands: &[ImguiCommand], ui: &Ui, outputs: &crate::WidgetOutputs) {
    // Process commands, handling tree node groups recursively.
    // Widget output values are written to `outputs` so Lua can read them next frame.
    let mut i = 0;
    while i < commands.len() {
        match &commands[i] {
            ImguiCommand::TreeNodeBegin { label } => {
                let start = i + 1;
                let mut depth = 1;
                let mut end = start;
                while end < commands.len() {
                    match &commands[end] {
                        ImguiCommand::TreeNodeBegin { .. } => depth += 1,
                        ImguiCommand::TreeNodeEnd if depth == 1 => break,
                        ImguiCommand::TreeNodeEnd => depth -= 1,
                        _ => {}
                    }
                    end += 1;
                }
                let inner = &commands[start..end];
                if let Some(_token) = ui.tree_node(label) {
                    replay_inner(inner, ui, outputs);
                }
                i = end + 1;
            }
            ImguiCommand::TreeNodeEnd => {
                i += 1;
            }
            ImguiCommand::Text { text } => {
                ui.text(text);
                i += 1;
            }
            ImguiCommand::Button { label } => {
                ui.button(label);
                i += 1;
            }
            ImguiCommand::SameLine => {
                ui.same_line();
                i += 1;
            }
            ImguiCommand::Separator => {
                ui.separator();
                i += 1;
            }
            ImguiCommand::SliderFloat { label, min, max, value } => {
                let mut v = *value;
                ui.slider(label, *min, *max, &mut v);
                // Only store if user actually dragged (value changed).
                // If unchanged, remove stale output so external changes take effect.
                let mut out = outputs.lock().unwrap();
                if (v - *value).abs() > f32::EPSILON {
                    out.insert(label.clone(), crate::WidgetValue::Float(v));
                } else {
                    out.remove(label);
                }
                i += 1;
            }
            ImguiCommand::Checkbox { label, checked } => {
                let mut c = *checked;
                ui.checkbox(label, &mut c);
                let mut out = outputs.lock().unwrap();
                if c != *checked {
                    out.insert(label.clone(), crate::WidgetValue::Bool(c));
                } else {
                    out.remove(label);
                }
                i += 1;
            }
            ImguiCommand::ColorEdit3 { label, color } => {
                let mut c = *color;
                ui.color_edit3(label, &mut c);
                let mut out = outputs.lock().unwrap();
                if c != *color {
                    out.insert(label.clone(), crate::WidgetValue::Color3(c));
                } else {
                    out.remove(label);
                }
                i += 1;
            }
            ImguiCommand::InputText { label, text } => {
                let mut buf = text.clone();
                ui.input_text(label, &mut buf).build();
                i += 1;
            }
            ImguiCommand::CollapsingHeaderBegin { label, default_open } => {
                let start = i + 1;
                let mut depth = 1;
                let mut end = start;
                while end < commands.len() {
                    match &commands[end] {
                        ImguiCommand::CollapsingHeaderBegin { .. } => depth += 1,
                        ImguiCommand::CollapsingHeaderEnd if depth == 1 => break,
                        ImguiCommand::CollapsingHeaderEnd => depth -= 1,
                        _ => {}
                    }
                    end += 1;
                }
                let inner = &commands[start..end];
                let flags = if *default_open {
                    dear_imgui_rs::TreeNodeFlags::DEFAULT_OPEN
                } else {
                    dear_imgui_rs::TreeNodeFlags::empty()
                };
                if ui.collapsing_header(label, flags) {
                    replay_inner(inner, ui, outputs);
                }
                i = end + 1;
            }
            ImguiCommand::CollapsingHeaderEnd => {
                i += 1;
            }
            ImguiCommand::TextDisabled { text } => {
                ui.text_disabled(text);
                i += 1;
            }
            ImguiCommand::Indent => {
                ui.indent();
                i += 1;
            }
            ImguiCommand::Unindent => {
                ui.unindent();
                i += 1;
            }
            ImguiCommand::BeginWindow { .. } | ImguiCommand::EndWindow => {
                i += 1;
            }
        }
    }
}
