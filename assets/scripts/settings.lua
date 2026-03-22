-- Settings UI implemented in Lua via avatar SDK bindings.
-- Mirrors the Rust "Settings" panel using collapsing_header (not tree_node).

function settings_update(dt)
    imgui.begin("Settings (Lua)")

    -- ── Info (default open) ──
    imgui.collapsing_header("Info", true)
    imgui.text("Render FPS: " .. avatar.get_fps())
    imgui.text("Decode FPS: " .. avatar.get_decode_fps())
    imgui.text(string.format("Frame: %.1f ms", avatar.get_frame_ms()))
    imgui.text("Shading: " .. avatar.get_shading_mode())
    imgui.text("Idle Anim: " .. avatar.get_idle_anim_status())
    imgui.text(string.format("Camera dist: %.2f", avatar.get_camera_distance()))
    imgui.text("ImGui: " .. avatar.get_imgui_version())
    imgui.collapsing_header_end()

    -- ── Display (default open) ──
    imgui.collapsing_header("Display", true)

    local mascot = imgui.checkbox("Mascot Mode (M)", avatar.get_mascot_mode())
    avatar.set_mascot_mode(mascot)

    local aot = imgui.checkbox("Always on Top (F)", avatar.get_always_on_top())
    avatar.set_always_on_top(aot)

    local fs = imgui.checkbox("Maximized Window", avatar.get_fullscreen())
    avatar.set_fullscreen(fs)

    local dbg = imgui.checkbox("Debug Overlay", avatar.get_debug_overlay())
    avatar.set_debug_overlay(dbg)

    local dist = imgui.slider_float("Camera Distance", 0.5, 10.0, avatar.get_camera_distance())
    avatar.set_camera_distance(dist)

    local aot = imgui.checkbox("Avatar on Top", avatar.get_avatar_on_top())
    avatar.set_avatar_on_top(aot)

    imgui.collapsing_header_end()

    -- ── Tracking (default open) ──
    imgui.collapsing_header("Tracking", true)

    local tr = imgui.checkbox("Tracking (T)", avatar.get_tracking())
    avatar.set_tracking(tr)

    imgui.separator()
    imgui.text("Features:")

    local ft = imgui.checkbox("Face Tracking", avatar.get_face_tracking())
    avatar.set_face_tracking(ft)

    local at = imgui.checkbox("Arm Tracking", avatar.get_arm_tracking())
    avatar.set_arm_tracking(at)

    local ht = imgui.checkbox("Hand Tracking", avatar.get_hand_tracking())
    avatar.set_hand_tracking(ht)

    imgui.separator()

    local ab = imgui.checkbox("Auto Blink (B)", avatar.get_auto_blink())
    avatar.set_auto_blink(ab)

    local ia = imgui.checkbox("Idle Animation (I)", avatar.get_idle_animation())
    avatar.set_idle_animation(ia)

    local vc = imgui.checkbox("Virtual Camera (C)", avatar.get_vcam())
    avatar.set_vcam(vc)

    local vl = imgui.checkbox("VirtualLive Shading", avatar.get_virtual_live_shading())
    avatar.set_virtual_live_shading(vl)

    imgui.collapsing_header_end()

    -- ── Lighting (default open) ──
    imgui.collapsing_header("Lighting", true)

    local ki = imgui.slider_float("Key Intensity", 0.0, 3.0, avatar.get_light_intensity("key"))
    avatar.set_light_intensity("key", ki)

    local kr, kg, kb = avatar.get_light_color("key")
    imgui.color_edit3("Key Color", kr, kg, kb)

    imgui.separator()

    local fi = imgui.slider_float("Fill Intensity", 0.0, 3.0, avatar.get_light_intensity("fill"))
    avatar.set_light_intensity("fill", fi)

    local fr, fg, fb = avatar.get_light_color("fill")
    imgui.color_edit3("Fill Color", fr, fg, fb)

    imgui.separator()

    local bi = imgui.slider_float("Back Intensity", 0.0, 3.0, avatar.get_light_intensity("back"))
    avatar.set_light_intensity("back", bi)

    local br, bg, bb = avatar.get_light_color("back")
    imgui.color_edit3("Back Color", br, bg, bb)

    imgui.collapsing_header_end()

    imgui.end_window()
end
