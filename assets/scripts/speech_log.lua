-- Speech Recognition Log Window
-- Displays real-time transcription results from Whisper STT.

local function draw_speech_log(dt)
    imgui.begin("Speech Log")

    -- VAD status indicator + Reset button
    local vad = avatar.get_vad_active()
    if vad then
        imgui.text("[VAD] Speaking...")
    else
        imgui.text_disabled("[VAD] Idle")
    end

    imgui.same_line()
    if imgui.button("Reset VAD") then
        avatar.reset_speech()
    end

    -- Interim (partial) transcript
    local interim = avatar.get_speech_interim()
    if interim and interim ~= "" then
        imgui.text(">> " .. interim)
    end

    imgui.separator()

    -- Transcript log entries
    local entries = avatar.get_speech_log()
    if entries then
        for i = #entries, 1, -1 do
            imgui.text(entries[i])
        end
    else
        imgui.text_disabled("No transcripts yet")
    end

    imgui.end_window()
end

-- Register as global so ui.lua can call it
speech_log_update = draw_speech_log
