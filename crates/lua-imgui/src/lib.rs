//! Lua scripting for Dear ImGui — command buffer approach.
//!
//! `LuaImgui` does NOT own an ImGui context or renderer. Instead it:
//! 1. Collects ImGui commands from Lua scripts into a buffer
//! 2. Replays them against an existing `&dear_imgui_rs::Ui` during the frame
//! 3. Feeds widget output values back to Lua via a shared map
//!
//! The host application (app crate) owns the ImGui context via `ImGuiRenderer`
//! and calls `lua_imgui.replay(ui)` inside `frame_with_nodes()`.

pub mod bindings;
pub mod commands;
pub mod events;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use lua_runtime::LuaRuntime;

/// Widget output values stored after replay, keyed by widget label.
/// These are fed back to Lua on the next frame so sliders/checkboxes
/// return user-modified values instead of always returning the initial value.
#[derive(Debug, Clone)]
pub enum WidgetValue {
    Float(f32),
    Bool(bool),
    Color3([f32; 3]),
    Text(String),
}

/// Shared widget output map — written by replay, read by Lua bindings.
pub type WidgetOutputs = Arc<Mutex<HashMap<String, WidgetValue>>>;

/// Lua-driven Dear ImGui command buffer.
///
/// Does not own an ImGui context — commands are replayed against an external `Ui`.
pub struct LuaImgui {
    lua: LuaRuntime,
    commands: Arc<Mutex<Vec<commands::ImguiCommand>>>,
    /// Widget output values from last replay (fed back to Lua).
    pub widget_outputs: WidgetOutputs,
    /// Lua window visibility: window name → visible.
    pub window_visibility: HashMap<String, bool>,
}

impl LuaImgui {
    /// Create a new Lua-ImGui command buffer (no ImGui context created).
    pub fn new() -> Result<Self> {
        let lua = LuaRuntime::new()?;

        let commands = Arc::new(Mutex::new(Vec::new()));
        let widget_outputs: WidgetOutputs = Arc::new(Mutex::new(HashMap::new()));
        bindings::register(&lua, commands.clone(), widget_outputs.clone())?;

        Ok(Self {
            lua,
            commands,
            widget_outputs,
            window_visibility: HashMap::new(),
        })
    }

    /// Load a Lua UI script.
    pub fn load_script(&self, path: &std::path::Path) -> Result<()> {
        self.lua.exec_file(path)
    }

    /// Access the Lua runtime for custom bindings.
    pub fn lua(&self) -> &LuaRuntime {
        &self.lua
    }

    /// Call Lua `update(dt)` to collect commands, then replay them on the Ui.
    pub fn replay(&mut self, ui: &dear_imgui_rs::Ui, dt: f32) {
        self.commands.lock().unwrap().clear();

        if let Err(e) = self.lua.call_global::<_, ()>("update", dt) {
            let msg = format!("{e}");
            if !msg.contains("not found") && !msg.contains("is not a function") {
                log::warn!("Lua update error: {e}");
            }
        }

        let cmds: Vec<_> = self.commands.lock().unwrap().clone();
        let outputs = self.widget_outputs.clone();
        self.replay_nested_filtered(&cmds, ui, &outputs);
    }

    fn replay_nested_filtered(
        &mut self,
        commands: &[commands::ImguiCommand],
        ui: &dear_imgui_rs::Ui,
        outputs: &WidgetOutputs,
    ) {
        let mut i = 0;
        while i < commands.len() {
            match &commands[i] {
                commands::ImguiCommand::BeginWindow { name } => {
                    let name = name.clone();
                    self.window_visibility.entry(name.clone()).or_insert(true);

                    let start = i + 1;
                    let mut depth = 1;
                    let mut end = start;
                    while end < commands.len() {
                        match &commands[end] {
                            commands::ImguiCommand::BeginWindow { .. } => depth += 1,
                            commands::ImguiCommand::EndWindow if depth == 1 => break,
                            commands::ImguiCommand::EndWindow => depth -= 1,
                            _ => {}
                        }
                        end += 1;
                    }

                    if *self.window_visibility.get(&name).unwrap_or(&true) {
                        let inner = &commands[start..end];
                        let mut opened = true;
                        let out = outputs.clone();
                        ui.window(&name).opened(&mut opened).build(|| {
                            commands::replay_inner(inner, ui, &out);
                        });
                        if !opened {
                            self.window_visibility.insert(name, false);
                        }
                    }

                    i = end + 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
    }
}
