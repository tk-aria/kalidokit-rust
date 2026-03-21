-- Example UI script for kalidokit-rust
-- This function is called every frame by the Lua-ImGui runtime.

local frame_count = 0
local camera_dist = 3.0
local show_debug = false

function update(dt)
    frame_count = frame_count + 1

    imgui.begin("Controls")
    imgui.text(string.format("Frame: %d  dt: %.1fms", frame_count, dt * 1000))
    imgui.separator()

    camera_dist = imgui.slider_float("Camera Distance", 0.5, 10.0, camera_dist)
    show_debug = imgui.checkbox("Show Debug Overlay", show_debug)

    if imgui.button("Reset") then
        camera_dist = 3.0
        show_debug = false
    end

    imgui.end_window()
end
