-- Task List — Notion Cabinet DB integration
-- Uses Rust backend (AvatarAction) for async API calls.

local completed_ids = {}
local input_buffers = {} -- per-task input text

local function draw_task_list(dt)
    imgui.begin("Task List")

    -- Header: Refresh + loading status
    local loading = avatar.get_notion_loading()
    if loading then
        imgui.text("Loading...")
    else
        local do_refresh = imgui.checkbox("Refresh", false)
        if do_refresh then
            avatar.notion_refresh()
        end
    end

    imgui.same_line()
    imgui.text("  " .. os.date("%Y-%m-%d"))

    local err = avatar.get_notion_error()
    if err and err ~= "" then
        imgui.text("[Error] " .. err)
    end

    imgui.separator()

    -- Task list from Rust backend
    local tasks = avatar.get_notion_tasks()
    if not tasks or #tasks == 0 then
        if not loading then
            imgui.text_disabled("No tasks. Click Refresh.")
        end
        imgui.end_window()
        return
    end

    for i, task in ipairs(tasks) do
        if not completed_ids[task.id] then
            -- Checkbox (Done) on the left
            local done_label = "##done_" .. i
            local checked = imgui.checkbox(done_label, false)
            if checked then
                completed_ids[task.id] = true
                avatar.notion_complete(task.id)
            end

            imgui.same_line()

            -- Build header: time (duration) [priority] title
            local header = ""
            if task.time ~= "" then header = task.time end
            if task.duration ~= "" then header = header .. " (" .. task.duration .. ")" end
            if task.priority ~= "" then header = header .. " [" .. task.priority .. "]" end
            header = header .. " " .. task.title .. "##task_" .. i

            -- Collapsing header (toggle)
            imgui.collapsing_header(header, false)

            -- Status
            if task.status ~= "" and task.status ~= "Not started" then
                imgui.text_disabled("  Status: " .. task.status)
            end

            -- Children
            local children = task.children
            if children and #children > 0 then
                for j, child in ipairs(children) do
                    imgui.text("    - " .. child.title)
                end
            end

            -- Sub-task input: text field + Enter to create
            if not input_buffers[task.id] then
                input_buffers[task.id] = ""
            end
            local input_label = "##subtask_" .. i
            input_buffers[task.id] = imgui.input_text(input_label, input_buffers[task.id])
            if imgui.input_text_submitted(input_label) then
                local text = input_buffers[task.id]
                if text ~= "" then
                    avatar.notion_create_child(task.id, text)
                    input_buffers[task.id] = ""
                end
            end

            imgui.collapsing_header_end()
        end
    end

    imgui.end_window()
end

-- Initial fetch on load
avatar.notion_refresh()

notion_tasks_update = draw_task_list
