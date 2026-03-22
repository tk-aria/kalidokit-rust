//! Register the `avatar` Lua table with get/set bindings for AvatarState.
//!
//! This module lives in the app crate (not lua-imgui) because it bridges
//! app-specific state with the Lua runtime. lua-imgui remains generic.

use std::sync::{Arc, Mutex};

use avatar_sdk::{ActionQueue, AvatarAction, AvatarState};
use lua_imgui::LuaImgui;

/// Shared handle to avatar state + action queue, used by both Lua and app.
#[derive(Clone)]
pub struct AvatarHandle {
    pub state: Arc<Mutex<AvatarState>>,
    pub actions: Arc<Mutex<ActionQueue>>,
}

impl AvatarHandle {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AvatarState::default())),
            actions: Arc::new(Mutex::new(ActionQueue::default())),
        }
    }
}

/// Register the `avatar` table on the Lua runtime provided by lua-imgui.
pub fn register(lua_imgui: &LuaImgui, handle: &AvatarHandle) -> anyhow::Result<()> {
    let l = lua_imgui.lua().lua();
    let avatar_table = l.create_table()?;

    // ── Info (read-only) ──

    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_fps",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().info.render_fps)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_decode_fps",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().info.decode_fps)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_frame_ms",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().info.frame_ms)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_shading_mode",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().info.shading_mode.clone())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_idle_anim_status",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().info.idle_anim_status.clone())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_imgui_version",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().info.imgui_version.clone())
            })?,
        )?;
    }

    // ── Display ──

    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_camera_distance",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().display.camera_distance)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_camera_distance",
            l.create_function(move |_, v: f32| {
                h.lock().unwrap().display.camera_distance = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_mascot_mode",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().display.mascot_enabled)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_mascot_mode",
            l.create_function(move |_, v: bool| {
                h.lock().unwrap().display.mascot_enabled = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_always_on_top",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().display.always_on_top)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_always_on_top",
            l.create_function(move |_, v: bool| {
                h.lock().unwrap().display.always_on_top = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_fullscreen",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().display.fullscreen)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_fullscreen",
            l.create_function(move |_, v: bool| {
                h.lock().unwrap().display.fullscreen = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_debug_overlay",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().display.debug_overlay)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_debug_overlay",
            l.create_function(move |_, v: bool| {
                h.lock().unwrap().display.debug_overlay = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_model_offset",
            l.create_function(move |_, ()| {
                let o = h.lock().unwrap().display.model_offset;
                Ok((o[0], o[1]))
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_model_offset",
            l.create_function(move |_, (x, y): (f32, f32)| {
                h.lock().unwrap().display.model_offset = [x, y];
                Ok(())
            })?,
        )?;
    }

    // ── Tracking ──

    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_tracking",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().tracking.tracking_enabled)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_tracking",
            l.create_function(move |_, v: bool| {
                h.lock().unwrap().tracking.tracking_enabled = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_auto_blink",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().tracking.auto_blink)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_auto_blink",
            l.create_function(move |_, v: bool| {
                h.lock().unwrap().tracking.auto_blink = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_idle_animation",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().tracking.idle_animation)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_idle_animation",
            l.create_function(move |_, v: bool| {
                h.lock().unwrap().tracking.idle_animation = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_vcam",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().tracking.vcam_enabled)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_vcam",
            l.create_function(move |_, v: bool| {
                h.lock().unwrap().tracking.vcam_enabled = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_virtual_live_shading",
            l.create_function(move |_, ()| {
                Ok(h.lock().unwrap().tracking.virtual_live_shading)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_virtual_live_shading",
            l.create_function(move |_, v: bool| {
                h.lock().unwrap().tracking.virtual_live_shading = v;
                Ok(())
            })?,
        )?;
    }

    // ── Lighting ──

    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_light_intensity",
            l.create_function(move |_, name: String| {
                let s = h.lock().unwrap();
                let light = match name.as_str() {
                    "key" => &s.lighting.key,
                    "fill" => &s.lighting.fill,
                    "back" => &s.lighting.back,
                    _ => return Ok(0.0),
                };
                Ok(light.intensity)
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_light_intensity",
            l.create_function(move |_, (name, v): (String, f32)| {
                let mut s = h.lock().unwrap();
                let light = match name.as_str() {
                    "key" => &mut s.lighting.key,
                    "fill" => &mut s.lighting.fill,
                    "back" => &mut s.lighting.back,
                    _ => return Ok(()),
                };
                light.intensity = v;
                Ok(())
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "get_light_color",
            l.create_function(move |_, name: String| {
                let s = h.lock().unwrap();
                let light = match name.as_str() {
                    "key" => &s.lighting.key,
                    "fill" => &s.lighting.fill,
                    "back" => &s.lighting.back,
                    _ => return Ok((0.0, 0.0, 0.0)),
                };
                Ok((light.color[0], light.color[1], light.color[2]))
            })?,
        )?;
    }
    {
        let h = handle.state.clone();
        avatar_table.set(
            "set_light_color",
            l.create_function(move |_, (name, r, g, b): (String, f32, f32, f32)| {
                let mut s = h.lock().unwrap();
                let light = match name.as_str() {
                    "key" => &mut s.lighting.key,
                    "fill" => &mut s.lighting.fill,
                    "back" => &mut s.lighting.back,
                    _ => return Ok(()),
                };
                light.color = [r, g, b];
                Ok(())
            })?,
        )?;
    }

    // ── Actions ──

    {
        let a = handle.actions.clone();
        avatar_table.set(
            "set_background",
            l.create_function(move |_, path: String| {
                a.lock().unwrap().push(AvatarAction::ApplyBackgroundImage(path));
                Ok(())
            })?,
        )?;
    }
    {
        let a = handle.actions.clone();
        avatar_table.set(
            "browse_background",
            l.create_function(move |_, ()| {
                a.lock().unwrap().push(AvatarAction::BrowseBackgroundImage);
                Ok(())
            })?,
        )?;
    }

    l.globals().set("avatar", avatar_table)?;
    Ok(())
}
