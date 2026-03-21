//! Register imgui API functions into the Lua global namespace.

use std::sync::{Arc, Mutex};

use lua_runtime::LuaRuntime;

use crate::commands::ImguiCommand;

type CmdBuf = Arc<Mutex<Vec<ImguiCommand>>>;

/// Register the `imgui` table with all API functions.
pub fn register(lua: &LuaRuntime, commands: CmdBuf) -> anyhow::Result<()> {
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
    {
        let cmds = commands.clone();
        imgui_table.set(
            "slider_float",
            l.create_function(move |_, (label, min, max, value): (String, f32, f32, f32)| {
                cmds.lock().unwrap().push(ImguiCommand::SliderFloat {
                    label,
                    min,
                    max,
                    value,
                });
                Ok(value) // return current value (will be updated next frame)
            })?,
        )?;
    }

    // imgui.checkbox(label, checked) -> checked
    {
        let cmds = commands.clone();
        imgui_table.set(
            "checkbox",
            l.create_function(move |_, (label, checked): (String, bool)| {
                cmds.lock().unwrap().push(ImguiCommand::Checkbox { label, checked });
                Ok(checked)
            })?,
        )?;
    }

    // imgui.color_edit3(label, r, g, b) -> r, g, b
    {
        let cmds = commands.clone();
        imgui_table.set(
            "color_edit3",
            l.create_function(move |_, (label, r, g, b): (String, f32, f32, f32)| {
                cmds.lock().unwrap().push(ImguiCommand::ColorEdit3 {
                    label,
                    color: [r, g, b],
                });
                Ok((r, g, b))
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

    l.globals().set("imgui", imgui_table)?;
    Ok(())
}
