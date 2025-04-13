//! Notion API client for Cabinet DB task management.
//! Uses curl via std::process::Command (no additional dependencies).

use avatar_sdk::{NotionChild, NotionTask};

fn notion_token() -> String {
    std::env::var("NOTION_TOKEN").unwrap_or_default()
}

fn cabinet_db_id() -> String {
    std::env::var("NOTION_CABINET_DB_ID")
        .unwrap_or_else(|_| "3f36c137-cecd-410a-95f9-e7f867da1b4d".to_string())
}
const API_BASE: &str = "https://api.notion.com/v1";

fn curl_post(url: &str, body: &str) -> Result<String, String> {
    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-X", "POST",
            "-H", &format!("Authorization: Bearer {}", notion_token()),
            "-H", "Notion-Version: 2022-06-28",
            "-H", "Content-Type: application/json",
            url,
            "-d", body,
        ])
        .output()
        .map_err(|e| format!("curl exec failed: {e}"))?;
    String::from_utf8(output.stdout).map_err(|e| format!("utf8 error: {e}"))
}

fn curl_patch(url: &str, body: &str) -> Result<String, String> {
    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-X", "PATCH",
            "-H", &format!("Authorization: Bearer {}", notion_token()),
            "-H", "Notion-Version: 2022-06-28",
            "-H", "Content-Type: application/json",
            url,
            "-d", body,
        ])
        .output()
        .map_err(|e| format!("curl exec failed: {e}"))?;
    String::from_utf8(output.stdout).map_err(|e| format!("utf8 error: {e}"))
}

/// Fetch today's tasks from Cabinet DB.
pub fn fetch_today_tasks() -> Result<Vec<NotionTask>, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let body = format!(
        r#"{{"filter":{{"property":"Date","date":{{"equals":"{today}"}}}}}}"#
    );
    let url = format!("{API_BASE}/databases/{}/query", cabinet_db_id());
    let json_str = curl_post(&url, &body)?;

    let json: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("JSON parse error: {e}"))?;

    let results = json["results"].as_array().ok_or("no results array")?;
    let mut tasks: Vec<NotionTask> = Vec::new();

    for page in results {
        let id = page["id"].as_str().unwrap_or("").to_string();
        let props = &page["properties"];

        // Title
        let title = props["Name"]["title"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|t| t["plain_text"].as_str())
            .unwrap_or("")
            .to_string();

        // Date
        let date_start = props["Date"]["date"]["start"].as_str().unwrap_or("");
        let date_end = props["Date"]["date"]["end"].as_str();

        // Time HH:MM
        let time = if let Some(cap) = date_start.find('T') {
            date_start[cap + 1..cap + 6].to_string()
        } else {
            String::new()
        };

        // Duration
        let duration = match date_end {
            Some(end) if !end.is_empty() => {
                if let (Some(st), Some(et)) = (date_start.find('T'), end.find('T')) {
                    let sh: i32 = date_start[st + 1..st + 3].parse().unwrap_or(0);
                    let sm: i32 = date_start[st + 4..st + 6].parse().unwrap_or(0);
                    let eh: i32 = end[et + 1..et + 3].parse().unwrap_or(0);
                    let em: i32 = end[et + 4..et + 6].parse().unwrap_or(0);
                    let dur_min = (eh * 60 + em) - (sh * 60 + sm);
                    if dur_min > 0 {
                        if dur_min >= 60 {
                            format!("{}h{:02}m", dur_min / 60, dur_min % 60)
                        } else {
                            format!("{dur_min}m")
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };

        // Status
        let status = props["Status"]["status"]["name"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Priority
        let priority = props["優先度"]["select"]["name"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if !title.is_empty() {
            tasks.push(NotionTask {
                id,
                title,
                time,
                duration,
                priority,
                status,
                children: Vec::new(),
            });
        }
    }

    // Sort by time, then priority
    tasks.sort_by(|a, b| a.time.cmp(&b.time).then(a.priority.cmp(&b.priority)));

    // Fetch children for each task
    for task in &mut tasks {
        if let Ok(children) = fetch_children(&task.id) {
            task.children = children;
        }
    }

    Ok(tasks)
}

fn fetch_children(page_id: &str) -> Result<Vec<NotionChild>, String> {
    let url = format!("{API_BASE}/blocks/{page_id}/children?page_size=100");
    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-H", &format!("Authorization: Bearer {}", notion_token()),
            "-H", "Notion-Version: 2022-06-28",
            &url,
        ])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    let json_str = String::from_utf8(output.stdout).map_err(|e| format!("utf8: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("JSON: {e}"))?;

    let mut children = Vec::new();
    if let Some(results) = json["results"].as_array() {
        for block in results {
            let id = block["id"].as_str().unwrap_or("").to_string();
            // Extract text from various block types
            for btype in &[
                "paragraph",
                "to_do",
                "bulleted_list_item",
                "numbered_list_item",
            ] {
                if let Some(rich_text) = block[btype]["rich_text"].as_array() {
                    let text: String = rich_text
                        .iter()
                        .filter_map(|t| t["plain_text"].as_str())
                        .collect();
                    if !text.is_empty() {
                        children.push(NotionChild {
                            id: id.clone(),
                            title: text,
                        });
                    }
                }
            }
        }
    }
    Ok(children)
}

/// Complete a task (set Status to Done).
pub fn complete_task(page_id: &str) -> Result<(), String> {
    let url = format!("{API_BASE}/pages/{page_id}");
    let body = r#"{"properties":{"Status":{"status":{"name":"Done"}}}}"#;
    curl_patch(&url, body)?;
    Ok(())
}

/// Create a child page under a task.
pub fn create_child_page(parent_id: &str, title: &str) -> Result<(), String> {
    let url = format!("{API_BASE}/pages");
    let body = format!(
        r#"{{"parent":{{"page_id":"{parent_id}"}},"properties":{{"title":{{"title":[{{"text":{{"content":"{title}"}}}}]}}}}}}"#
    );
    curl_post(&url, &body)?;
    Ok(())
}
