//! Codex CLI conversation parser.
//!
//! Converts Codex JSONL session files into the existing `ConversationEvent`
//! model so the rest of CluEdit (viewer, export, search) works unchanged.

use crate::models::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ============================================================================
// Helper constructors
// ============================================================================

fn make_user_event(content: MessageContent, timestamp: Option<String>) -> ConversationEvent {
    ConversationEvent::User {
        message: ApiMessage {
            role: "user".to_string(),
            content,
            model: None,
            id: None,
            stop_reason: None,
            stop_sequence: None,
            usage: None,
        },
        uuid: None,
        parent_uuid: None,
        session_id: None,
        timestamp,
        is_meta: false,
        is_sidechain: false,
        cwd: None,
        version: None,
        git_branch: None,
        slug: None,
        logical_parent_uuid: None,
        prompt_id: None,
        permission_mode: None,
        is_compact_summary: false,
        agent_id: None,
        source_tool_use_id: None,
        source_tool_assistant_uuid: None,
        tool_use_result: None,
        extra: serde_json::Map::new(),
    }
}

fn make_assistant_event(content: MessageContent, timestamp: Option<String>) -> ConversationEvent {
    ConversationEvent::Assistant {
        message: ApiMessage {
            role: "assistant".to_string(),
            content,
            model: None,
            id: None,
            stop_reason: None,
            stop_sequence: None,
            usage: None,
        },
        uuid: None,
        parent_uuid: None,
        session_id: None,
        timestamp,
        is_meta: false,
        is_sidechain: false,
        request_id: None,
        cwd: None,
        version: None,
        slug: None,
        agent_id: None,
        source_tool_use_id: None,
        source_tool_assistant_uuid: None,
        tool_use_result: None,
        is_api_error_message: false,
        error: None,
        thinking_metadata: None,
        logical_parent_uuid: None,
        extra: serde_json::Map::new(),
    }
}

// ============================================================================
// Codex JSONL line parsing → ConversationEvent
// ============================================================================

/// Parse a single Codex JSONL line into a ConversationEvent.
/// Returns None for lines that should be skipped (reasoning, token_count, etc.).
pub fn parse_codex_line(line: &str) -> Option<ConversationEvent> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let timestamp = v
        .get("timestamp")
        .and_then(|t| t.as_str())
        .map(String::from);
    let event_type = v.get("type").and_then(|t| t.as_str())?;
    let payload = v.get("payload")?;

    match event_type {
        "event_msg" => parse_event_msg(payload, timestamp),
        "response_item" => parse_response_item(payload, timestamp),
        _ => None,
    }
}

fn parse_event_msg(
    payload: &serde_json::Value,
    timestamp: Option<String>,
) -> Option<ConversationEvent> {
    let msg_type = payload.get("type").and_then(|t| t.as_str())?;
    match msg_type {
        "user_message" => {
            let text = payload.get("message").and_then(|m| m.as_str())?;
            if text.trim().is_empty() {
                return None;
            }
            Some(make_user_event(
                MessageContent::Text(text.to_string()),
                timestamp,
            ))
        }
        _ => None,
    }
}

fn parse_response_item(
    payload: &serde_json::Value,
    timestamp: Option<String>,
) -> Option<ConversationEvent> {
    let item_type = payload.get("type").and_then(|t| t.as_str())?;
    match item_type {
        "message" => {
            let role = payload.get("role").and_then(|r| r.as_str())?;
            if role == "assistant" {
                parse_assistant_message(payload, timestamp)
            } else {
                None
            }
        }
        "function_call" => parse_function_call(payload, timestamp),
        "function_call_output" => parse_function_call_output(payload, timestamp),
        "custom_tool_call" => parse_custom_tool_call(payload, timestamp),
        "custom_tool_call_output" => parse_custom_tool_call_output(payload, timestamp),
        _ => None,
    }
}

fn parse_assistant_message(
    payload: &serde_json::Value,
    timestamp: Option<String>,
) -> Option<ConversationEvent> {
    let content = payload.get("content").and_then(|c| c.as_array())?;
    let mut blocks: Vec<ContentBlock> = Vec::new();
    for block in content {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if matches!(block_type, "output_text" | "input_text") {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    blocks.push(ContentBlock::Text {
                        text: trimmed.to_string(),
                    });
                }
            }
        }
    }
    if blocks.is_empty() {
        return None;
    }
    Some(make_assistant_event(
        MessageContent::Blocks(blocks),
        timestamp,
    ))
}

fn parse_function_call(
    payload: &serde_json::Value,
    timestamp: Option<String>,
) -> Option<ConversationEvent> {
    let name = payload
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("unknown")
        .to_string();
    let call_id = payload
        .get("call_id")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let arguments = payload
        .get("arguments")
        .and_then(|a| a.as_str())
        .unwrap_or("{}");
    let input: serde_json::Value = serde_json::from_str(arguments)
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    Some(make_assistant_event(
        MessageContent::Blocks(vec![ContentBlock::ToolUse {
            id: call_id,
            name,
            input,
        }]),
        timestamp,
    ))
}

fn parse_function_call_output(
    payload: &serde_json::Value,
    timestamp: Option<String>,
) -> Option<ConversationEvent> {
    let call_id = payload
        .get("call_id")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let output = payload
        .get("output")
        .and_then(|o| o.as_str())
        .unwrap_or("")
        .to_string();

    Some(make_user_event(
        MessageContent::Blocks(vec![ContentBlock::ToolResult {
            tool_use_id: call_id,
            content: Some(serde_json::Value::String(output)),
            is_error: false,
        }]),
        timestamp,
    ))
}

fn parse_custom_tool_call(
    payload: &serde_json::Value,
    timestamp: Option<String>,
) -> Option<ConversationEvent> {
    let name = payload
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("unknown")
        .to_string();
    let call_id = payload
        .get("call_id")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let input_str = payload.get("input").and_then(|i| i.as_str()).unwrap_or("");

    Some(make_assistant_event(
        MessageContent::Blocks(vec![ContentBlock::ToolUse {
            id: call_id,
            name,
            input: serde_json::json!({ "patch": input_str }),
        }]),
        timestamp,
    ))
}

fn parse_custom_tool_call_output(
    payload: &serde_json::Value,
    timestamp: Option<String>,
) -> Option<ConversationEvent> {
    let call_id = payload
        .get("call_id")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let output = payload
        .get("output")
        .and_then(|o| o.as_str())
        .unwrap_or("")
        .to_string();

    Some(make_user_event(
        MessageContent::Blocks(vec![ContentBlock::ToolResult {
            tool_use_id: call_id,
            content: Some(serde_json::Value::String(output)),
            is_error: false,
        }]),
        timestamp,
    ))
}

// ============================================================================
// Session metadata extraction
// ============================================================================

#[allow(dead_code)]
pub struct CodexSessionMeta {
    pub id: String,
    pub cwd: String,
    pub timestamp: String,
}

pub fn read_session_meta(path: &Path) -> Option<CodexSessionMeta> {
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let first_line = reader.lines().next()?.ok()?;
    let v: serde_json::Value = serde_json::from_str(&first_line).ok()?;

    if v.get("type").and_then(|t| t.as_str()) != Some("session_meta") {
        return None;
    }

    let payload = v.get("payload")?;
    Some(CodexSessionMeta {
        id: payload.get("id").and_then(|i| i.as_str())?.to_string(),
        cwd: payload.get("cwd").and_then(|c| c.as_str())?.to_string(),
        timestamp: payload
            .get("timestamp")
            .or_else(|| v.get("timestamp"))
            .and_then(|t| t.as_str())?
            .to_string(),
    })
}

// ============================================================================
// History / title lookup
// ============================================================================

pub fn load_codex_history(codex_dir: &Path) -> HashMap<String, String> {
    let history_path = codex_dir.join("history.jsonl");
    let mut titles: HashMap<String, String> = HashMap::new();

    let file = match std::fs::File::open(&history_path) {
        Ok(f) => f,
        Err(_) => return titles,
    };
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(|l| l.ok()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            if let (Some(id), Some(text)) = (
                v.get("session_id").and_then(|s| s.as_str()),
                v.get("text").and_then(|t| t.as_str()),
            ) {
                let title = if text.len() > 120 {
                    let end = text
                        .char_indices()
                        .take(120)
                        .last()
                        .map(|(i, c)| i + c.len_utf8())
                        .unwrap_or(text.len());
                    format!("{}...", &text[..end])
                } else {
                    text.to_string()
                };
                titles.entry(id.to_string()).or_insert(title);
            }
        }
    }
    titles
}

// ============================================================================
// Project discovery (group sessions by cwd)
// ============================================================================

pub fn list_codex_projects(codex_dir: &Path) -> Vec<ProjectInfo> {
    let sessions_dir = codex_dir.join("sessions");
    if !sessions_dir.exists() {
        return Vec::new();
    }

    let mut project_map: HashMap<String, (PathBuf, usize)> = HashMap::new();

    for entry in WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(meta) = read_session_meta(path) {
            let project_name = Path::new(&meta.cwd)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let entry = project_map
                .entry(project_name)
                .or_insert_with(|| (PathBuf::from(&meta.cwd), 0));
            entry.1 += 1;
        }
    }

    let mut projects: Vec<ProjectInfo> = project_map
        .into_iter()
        .map(|(name, (cwd_path, count))| ProjectInfo {
            name,
            path: cwd_path,
            conversation_count: count,
            provider: Provider::Codex,
        })
        .collect();

    projects.sort_by(|a, b| a.name.cmp(&b.name));
    projects
}

pub fn codex_sessions_for_project(codex_dir: &Path, project_cwd: &str) -> Vec<PathBuf> {
    let sessions_dir = codex_dir.join("sessions");
    if !sessions_dir.exists() {
        return Vec::new();
    }

    let mut paths = Vec::new();
    for entry in WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(meta) = read_session_meta(path) {
            if meta.cwd == project_cwd {
                paths.push(path.to_path_buf());
            }
        }
    }

    paths.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_user_message() {
        let line = r#"{"timestamp":"2026-03-22T20:42:35.055Z","type":"event_msg","payload":{"type":"user_message","message":"hello world","images":[],"local_images":[],"text_elements":[]}}"#;
        let event = parse_codex_line(line).unwrap();
        assert_eq!(event.role(), Some("user"));
        assert_eq!(event.message_text().unwrap(), "hello world");
    }

    #[test]
    fn test_parse_assistant_message() {
        let line = r#"{"timestamp":"2026-03-22T20:42:47.864Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Here is the answer."}],"phase":"commentary"}}"#;
        let event = parse_codex_line(line).unwrap();
        assert_eq!(event.role(), Some("assistant"));
        assert_eq!(event.message_text().unwrap(), "Here is the answer.");
    }

    #[test]
    fn test_parse_function_call() {
        let line = r#"{"timestamp":"2026-03-22T20:42:47.864Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls\"}","call_id":"call_abc123"}}"#;
        let event = parse_codex_line(line).unwrap();
        assert_eq!(event.role(), Some("assistant"));
        assert!(event.is_chat_message());
    }

    #[test]
    fn test_parse_function_call_output() {
        let line = r#"{"timestamp":"2026-03-22T20:42:47.932Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_abc123","output":"file1.txt\nfile2.txt"}}"#;
        let event = parse_codex_line(line).unwrap();
        assert_eq!(event.role(), Some("user"));
    }

    #[test]
    fn test_skip_reasoning() {
        let line = r#"{"timestamp":"2026-03-22T20:42:43.074Z","type":"response_item","payload":{"type":"reasoning","summary":[],"content":null}}"#;
        assert!(parse_codex_line(line).is_none());
    }

    #[test]
    fn test_skip_developer_message() {
        let line = r#"{"timestamp":"2026-03-22T20:42:35.055Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"system prompt"}]}}"#;
        assert!(parse_codex_line(line).is_none());
    }

    #[test]
    fn test_skip_token_count() {
        let line = r#"{"timestamp":"2026-03-22T20:42:36.730Z","type":"event_msg","payload":{"type":"token_count","info":null}}"#;
        assert!(parse_codex_line(line).is_none());
    }
}
