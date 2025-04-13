-- Test: can Lua call os.execute and io.popen?
-- Runs once at load time (not per-frame)

_G._shell_test = {}

-- Test 1: os.execute availability
if os and os.execute then
    _G._shell_test.os_execute = "Available"
else
    _G._shell_test.os_execute = "NOT available"
end

-- Test 2: io.popen availability + actual execution
if io and io.popen then
    _G._shell_test.io_popen = "Available"

    -- Try running a simple command at load time
    local handle = io.popen("echo hello_from_lua 2>&1")
    if handle then
        local result = handle:read("*a")
        handle:close()
        _G._shell_test.echo_result = result or "(nil)"
    else
        _G._shell_test.echo_result = "popen() returned nil"
    end

    -- Try running 'which claude' to check if claude CLI is available
    local h2 = io.popen("which claude 2>&1")
    if h2 then
        local r2 = h2:read("*a")
        h2:close()
        _G._shell_test.claude_path = r2 or "(nil)"
    else
        _G._shell_test.claude_path = "popen() returned nil"
    end
else
    _G._shell_test.io_popen = "NOT available"
    _G._shell_test.echo_result = "N/A"
    _G._shell_test.claude_path = "N/A"
end

-- Display results each frame
local function test_lua_shell(dt)
    imgui.begin("Lua Shell Test")

    imgui.text("[os.execute] " .. _G._shell_test.os_execute)
    imgui.text("[io.popen]   " .. _G._shell_test.io_popen)
    imgui.separator()
    imgui.text("echo result: " .. _G._shell_test.echo_result)
    imgui.text("claude path: " .. _G._shell_test.claude_path)

    imgui.end_window()
end

test_os_update = test_lua_shell
