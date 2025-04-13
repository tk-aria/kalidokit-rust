//! Register imgui API functions into the Lua global namespace.

use std::sync::{Arc, Mutex};

use lua_runtime::LuaRuntime;

use crate::commands::ImguiCommand;
use crate::{WidgetOutputs, WidgetValue};

type CmdBuf = Arc<Mutex<Vec<ImguiCommand>>>;

/// Register the `imgui` table with all API functions.
pub fn register(lua: &LuaRuntime, commands: CmdBuf, outputs: WidgetOutputs) -> anyhow::Result<()> {
    let l = lua.lua();
    let imgui_table = l.create_table()?;

    // imgui.begin(name)
    {
        let cmds = commands.clone();
        imgui_table.set(
            "begin",
            l.create_function(move |_, name: String| {
                cmds.lock().unwrap().push(ImguiCommand::BeginWindow { name });
                Ok(())
            })?,
        )?;
    }

    // imgui.end_window()
    {
        let cmds = commands.clone();
        imgui_table.set(
            "end_window",
            l.create_function(move |_, ()| {
                cmds.lock().unwrap().push(ImguiCommand::EndWindow);
                Ok(())
            })?,
        )?;
    }

    // imgui.text(str)
    {
        let cmds = commands.clone();
        imgui_table.set(
            "text",
            l.create_function(move |_, text: String| {
                cmds.lock().unwrap().push(ImguiCommand::Text { text });
                Ok(())
            })?,
        )?;
    }

    // imgui.button(label) -> bool (always false in command buffer mode)
    {
        let cmds = commands.clone();
        imgui_table.set(
            "button",
            l.create_function(move |_, label: String| {
                cmds.lock().unwrap().push(ImguiCommand::Button { label });
                Ok(false)
            })?,
        )?;
    }

    // imgui.same_line()
    {
        let cmds = commands.clone();
        imgui_table.set(
            "same_line",
            l.create_function(move |_, ()| {
                cmds.lock().unwrap().push(ImguiCommand::SameLine);
                Ok(())
            })?,
        )?;
    }

    // imgui.separator()
    {
        let cmds = commands.clone();
        imgui_table.set(
            "separator",
            l.create_function(move |_, ()| {
                cmds.lock().unwrap().push(ImguiCommand::Separator);
                Ok(())
            })?,
        )?;
    }

    // imgui.slider_float(label, min, max, value) -> value
    // Returns the user-modified value from last frame's replay (feedback loop).
    {
        let cmds = commands.clone();
        let out = outputs.clone();
        imgui_table.set(
            "slider_float",
            l.create_function(move |_, (label, min, max, value): (String, f32, f32, f32)| {
                // Read back the value from last frame's replay (if user dragged the slider)
                let actual = match out.lock().unwrap().get(&label) {
                    Some(WidgetValue::Float(v)) => *v,
                    _ => value,
                };
                cmds.lock().unwrap().push(ImguiCommand::SliderFloat {
                    label,
                    min,
                    max,
                    value: actual,
                });
                Ok(actual)
            })?,
        )?;
    }

    // imgui.checkbox(label, checked) -> checked
    {
        let cmds = commands.clone();
        let out = outputs.clone();
        imgui_table.set(
            "checkbox",
            l.create_function(move |_, (label, checked): (String, bool)| {
                let actual = match out.lock().unwrap().get(&label) {
                    Some(WidgetValue::Bool(v)) => *v,
                    _ => checked,
                };
                cmds.lock().unwrap().push(ImguiCommand::Checkbox { label, checked: actual });
                Ok(actual)
            })?,
        )?;
    }

    // imgui.color_edit3(label, r, g, b) -> r, g, b
    {
        let cmds = commands.clone();
        let out = outputs.clone();
        imgui_table.set(
            "color_edit3",
            l.create_function(move |_, (label, r, g, b): (String, f32, f32, f32)| {
                let (ar, ag, ab) = match out.lock().unwrap().get(&label) {
                    Some(WidgetValue::Color3(c)) => (c[0], c[1], c[2]),
                    _ => (r, g, b),
                };
                cmds.lock().unwrap().push(ImguiCommand::ColorEdit3 {
                    label,
                    color: [ar, ag, ab],
                });
                Ok((ar, ag, ab))
            })?,
        )?;
    }

    // imgui.tree_node(label)
    {
        let cmds = commands.clone();
        imgui_table.set(
            "tree_node",
            l.create_function(move |_, label: String| {
                cmds.lock().unwrap().push(ImguiCommand::TreeNodeBegin { label });
                Ok(())
            })?,
        )?;
    }

    // imgui.tree_pop()
    {
        let cmds = commands.clone();
        imgui_table.set(
            "tree_pop",
            l.create_function(move |_, ()| {
                cmds.lock().unwrap().push(ImguiCommand::TreeNodeEnd);
                Ok(())
            })?,
        )?;
    }

    // imgui.collapsing_header(label, default_open?)
    {
        let cmds = commands.clone();
        imgui_table.set(
            "collapsing_header",
            l.create_function(move |_, (label, default_open): (String, Option<bool>)| {
                cmds.lock().unwrap().push(ImguiCommand::CollapsingHeaderBegin {
                    label,
                    default_open: default_open.unwrap_or(false),
                });
                Ok(())
            })?,
        )?;
    }

    // imgui.collapsing_header_end()
    {
        let cmds = commands.clone();
        imgui_table.set(
            "collapsing_header_end",
            l.create_function(move |_, ()| {
                cmds.lock().unwrap().push(ImguiCommand::CollapsingHeaderEnd);
                Ok(())
            })?,
        )?;
    }

    // imgui.text_disabled(text)
    {
        let cmds = commands.clone();
        imgui_table.set(
            "text_disabled",
            l.create_function(move |_, text: String| {
                cmds.lock().unwrap().push(ImguiCommand::TextDisabled { text });
                Ok(())
            })?,
        )?;
    }

    // imgui.indent()
    {
        let cmds = commands.clone();
        imgui_table.set(
            "indent",
            l.create_function(move |_, ()| {
                cmds.lock().unwrap().push(ImguiCommand::Indent);
                Ok(())
            })?,
        )?;
    }

    // imgui.unindent()
    {
        let cmds = commands.clone();
        imgui_table.set(
            "unindent",
            l.create_function(move |_, ()| {
                cmds.lock().unwrap().push(ImguiCommand::Unindent);
                Ok(())
            })?,
        )?;
    }

    // imgui.input_text(label, text) -> text
    {
        let cmds = commands.clone();
        let out = outputs.clone();
        imgui_table.set(
            "input_text",
            l.create_function(move |_, (label, text): (String, String)| {
                let actual = match out.lock().unwrap().get(&label) {
                    Some(WidgetValue::Text(s)) => s.clone(),
                    _ => text.clone(),
                };
                cmds.lock().unwrap().push(ImguiCommand::InputText {
                    label,
                    text: actual.clone(),
                });
                Ok(actual)
            })?,
        )?;
    }

    // imgui.input_text_submitted(label) -> bool
    // Returns true for one frame when Enter was pressed on the input_text.
    {
        let out = outputs.clone();
        imgui_table.set(
            "input_text_submitted",
            l.create_function(move |_, label: String| {
                let key = format!("{label}__submitted");
                let submitted = matches!(
                    out.lock().unwrap().get(&key),
                    Some(WidgetValue::Bool(true))
                );
                Ok(submitted)
            })?,
        )?;
    }

    l.globals().set("imgui", imgui_table)?;
    Ok(())
}
