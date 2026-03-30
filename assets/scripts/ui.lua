-- Main UI entry point for kalidokit-rust Lua ImGui.
-- Calls all registered UI modules each frame.

local frame_count = 0

-- Controls window
local function draw_controls(dt)
    frame_count = frame_count + 1

    imgui.begin("Controls")
    imgui.text(string.format("Frame: %d  dt: %.1fms", frame_count, dt * 1000))
    imgui.separator()

    local dist = imgui.slider_float("Camera Distance", 0.5, 10.0, avatar.get_camera_distance())
    avatar.set_camera_distance(dist)

    local dbg = imgui.checkbox("Show Debug Overlay", avatar.get_debug_overlay())
    avatar.set_debug_overlay(dbg)

    if imgui.button("Reset") then
        avatar.set_camera_distance(3.0)
        avatar.set_debug_overlay(false)
    end

    imgui.end_window()
end

-- Global update — called by lua-imgui each frame
function update(dt)
    draw_controls(dt)

    -- Call settings UI if loaded
    if settings_update then
        settings_update(dt)
    end

    -- Call speech log if loaded
    if speech_log_update then
        speech_log_update(dt)
    end
end
