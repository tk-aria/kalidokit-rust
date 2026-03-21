//! ImGui command buffer — collected from Lua, replayed during imgui frame.

use imgui::Ui;

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
                ui.window(&name).build(|| {
                    replay_inner(inner, ui);
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

fn replay_inner(commands: &[ImguiCommand], ui: &Ui) {
    for cmd in commands {
        match cmd {
            ImguiCommand::Text { text } => ui.text(text),
            ImguiCommand::Button { label } => {
                ui.button(label);
            }
            ImguiCommand::SameLine => ui.same_line(),
            ImguiCommand::Separator => ui.separator(),
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
            ImguiCommand::ColorEdit3 { label, color } => {
                let mut c = *color;
                ui.color_edit3(label, &mut c);
            }
            _ => {}
        }
    }
}
