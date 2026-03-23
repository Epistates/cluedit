use regex::Regex;
use std::sync::OnceLock;

// ============================================================================
// Compiled regex patterns (cached via OnceLock, no external deps)
// ============================================================================

/// Messages consisting entirely of a slash command block
fn re_command_only() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?s)^\s*<command-name>.*?</command-name>\s*<command-message>.*?</command-message>\s*(<command-args>.*?</command-args>\s*)?$").unwrap()
    })
}

/// Messages consisting entirely of local-command-stdout
fn re_stdout_only() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?s)^\s*<local-command-stdout>.*?</local-command-stdout>\s*$").unwrap()
    })
}

/// Messages consisting entirely of local-command-caveat
fn re_caveat_only() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?s)^\s*<local-command-caveat>.*?</local-command-caveat>\s*$").unwrap()
    })
}

/// Task notification block (entire message)
fn re_task_notification_only() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?s)^\s*<task-notification>.*?</task-notification>\s*(Full transcript[^\n]*)?\s*$",
        )
        .unwrap()
    })
}

/// All known Claude XML tag pairs for inline stripping
fn re_claude_tags() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"(?s)<(?:",
            r"command-name|command-message|command-args|",
            r"local-command-caveat|local-command-stdout|local-command-stderr|",
            r"system-reminder|new-diagnostics|",
            r"available-deferred-tools|",
            r"task-notification|task-id|tool-use-id|output-file|",
            r"antml_thinking|thinking|",
            r"function_calls|antml_invoke|antml_parameter|",
            r"fast_mode_info|",
            r"user-prompt-submit-hook",
            r")(?:\s[^>]*)?>.*?</(?:",
            r"command-name|command-message|command-args|",
            r"local-command-caveat|local-command-stdout|local-command-stderr|",
            r"system-reminder|new-diagnostics|",
            r"available-deferred-tools|",
            r"task-notification|task-id|tool-use-id|output-file|",
            r"antml_thinking|thinking|",
            r"function_calls|antml_invoke|antml_parameter|",
            r"fast_mode_info|",
            r"user-prompt-submit-hook",
            r")>"
        ))
        .unwrap()
    })
}

/// Self-closing / unclosed Claude tags (e.g., <system-reminder> without closing)
fn re_claude_tags_self_closing() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"<(?:",
            r"system-reminder|new-diagnostics|available-deferred-tools|fast_mode_info",
            r")(?:\s[^>]*)?/>"
        ))
        .unwrap()
    })
}

/// Multiple consecutive blank lines
fn re_multi_blank_lines() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n{3,}").unwrap())
}

/// Low-value assistant narration patterns
fn re_low_value_narration() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(let me |now let me |i'll |now i'll |i'm going to |let me start |i need to |first,? (?:let me|i'll)|looking at )").unwrap()
    })
}

// ============================================================================
// Message-level filtering
// ============================================================================

/// Returns true if the entire message should be skipped (it's purely a Claude artifact).
pub fn should_skip_message(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }

    // Slash command blocks (/clear, /effort, etc.)
    if re_command_only().is_match(trimmed) {
        return true;
    }

    // Stdout-only messages (command output with no user content)
    if re_stdout_only().is_match(trimmed) {
        return true;
    }

    // Caveat-only messages
    if re_caveat_only().is_match(trimmed) {
        return true;
    }

    // Task notification blocks
    if re_task_notification_only().is_match(trimmed) {
        return true;
    }

    false
}

/// Returns true if an assistant message is low-value narration
/// (short tool-usage narration without substantive content).
pub fn is_low_value_assistant(text: &str) -> bool {
    let trimmed = text.trim();

    // Must be short
    if trimmed.len() > 200 {
        return false;
    }

    // Must not contain code blocks (those have actual content)
    if trimmed.contains("```") {
        return false;
    }

    // Check if it matches narration patterns
    re_low_value_narration().is_match(trimmed)
}

// ============================================================================
// Content sanitization
// ============================================================================

/// Maximum input size for sanitization (512KB). Larger inputs are truncated.
const MAX_SANITIZE_INPUT: usize = 512 * 1024;

/// Strip Claude-specific XML tags from text while preserving real content.
pub fn sanitize_for_training(text: &str) -> String {
    // Size cap to prevent excessive regex processing on pathological input
    let text = if text.len() > MAX_SANITIZE_INPUT {
        truncate_utf8(text, MAX_SANITIZE_INPUT)
    } else {
        text
    };
    let mut result = text.to_string();

    // Strip paired tags and their content
    result = re_claude_tags().replace_all(&result, "").to_string();

    // Strip self-closing tags
    result = re_claude_tags_self_closing()
        .replace_all(&result, "")
        .to_string();

    // Collapse multiple blank lines
    result = re_multi_blank_lines()
        .replace_all(&result, "\n\n")
        .to_string();

    result.trim().to_string()
}

// ============================================================================
// Turn merging
// ============================================================================

/// A single turn in a training conversation
pub struct Turn {
    pub role: String,
    pub content: String,
}

/// Merge consecutive turns with the same role into single turns.
/// Required for valid ChatML (alternating user/assistant) and ShareGPT (alternating human/gpt).
pub fn merge_consecutive_turns(turns: Vec<Turn>) -> Vec<Turn> {
    let mut merged: Vec<Turn> = Vec::new();

    for turn in turns {
        if let Some(last) = merged.last_mut() {
            if last.role == turn.role {
                last.content.push_str("\n\n");
                last.content.push_str(&turn.content);
                continue;
            }
        }
        merged.push(turn);
    }

    merged
}

// ============================================================================
// UTF-8 safe truncation
// ============================================================================

/// Truncate a string to at most `max_len` bytes at a valid UTF-8 char boundary.
pub fn truncate_utf8(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

// ============================================================================
// Tool use content formatting
// ============================================================================

/// Format tool use blocks into a human-readable string.
#[allow(dead_code)]
pub fn format_tool_use_block(name: &str, input: &serde_json::Value) -> String {
    let mut parts = vec![format!("[Tool: {}]", name)];

    // Extract the most useful input fields based on tool name
    if let Some(obj) = input.as_object() {
        match name {
            "Bash" => {
                if let Some(cmd) = obj.get("command").and_then(|v| v.as_str()) {
                    parts.push(format!("$ {}", cmd));
                }
            }
            "Read" => {
                if let Some(path) = obj.get("file_path").and_then(|v| v.as_str()) {
                    parts.push(format!("file: {}", path));
                }
            }
            "Write" => {
                if let Some(path) = obj.get("file_path").and_then(|v| v.as_str()) {
                    parts.push(format!("file: {}", path));
                }
                if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
                    // Truncate very long file writes for training
                    let preview = if content.len() > 500 {
                        format!(
                            "{}... ({} chars)",
                            truncate_utf8(content, 500),
                            content.len()
                        )
                    } else {
                        content.to_string()
                    };
                    parts.push(format!("```\n{}\n```", preview));
                }
            }
            "Edit" => {
                if let Some(path) = obj.get("file_path").and_then(|v| v.as_str()) {
                    parts.push(format!("file: {}", path));
                }
                if let Some(old) = obj.get("old_string").and_then(|v| v.as_str()) {
                    let preview = if old.len() > 200 {
                        format!("{}...", truncate_utf8(old, 200))
                    } else {
                        old.to_string()
                    };
                    parts.push(format!("old: ```{}```", preview));
                }
                if let Some(new) = obj.get("new_string").and_then(|v| v.as_str()) {
                    let preview = if new.len() > 200 {
                        format!("{}...", truncate_utf8(new, 200))
                    } else {
                        new.to_string()
                    };
                    parts.push(format!("new: ```{}```", preview));
                }
            }
            "Grep" | "Glob" => {
                if let Some(pattern) = obj.get("pattern").and_then(|v| v.as_str()) {
                    parts.push(format!("pattern: {}", pattern));
                }
                if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                    parts.push(format!("path: {}", path));
                }
            }
            _ => {
                // Generic: show compact JSON of input
                let compact = serde_json::to_string(input).unwrap_or_default();
                if compact.len() <= 300 {
                    parts.push(compact);
                }
            }
        }
    }

    parts.join(" ")
}

/// Format a tool result content block into a readable string.
#[allow(dead_code)]
pub fn format_tool_result(content: &Option<serde_json::Value>, is_error: bool) -> String {
    let prefix = if is_error { "[Error]" } else { "[Output]" };

    match content {
        Some(serde_json::Value::String(s)) => {
            let text = s.trim();
            if text.is_empty() {
                return String::new();
            }
            // Truncate very long outputs for training data
            let preview = if text.len() > 1000 {
                format!(
                    "{}... ({} chars total)",
                    truncate_utf8(text, 1000),
                    text.len()
                )
            } else {
                text.to_string()
            };
            format!("{} {}", prefix, preview)
        }
        Some(serde_json::Value::Array(arr)) => {
            // Tool results can be arrays of content blocks
            let texts: Vec<String> = arr
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(|t| t.as_str())
                        .map(|text| text.to_string())
                })
                .collect();
            if texts.is_empty() {
                return String::new();
            }
            let joined = texts.join("\n");
            let preview = if joined.len() > 1000 {
                format!(
                    "{}... ({} chars total)",
                    truncate_utf8(&joined, 1000),
                    joined.len()
                )
            } else {
                joined
            };
            format!("{} {}", prefix, preview)
        }
        _ => String::new(),
    }
}

// ============================================================================
// Project name sanitization
// ============================================================================

/// Clean up Claude project directory names for use in export filenames.
/// e.g., "-Users-nickpaterno-work-cluedit" → "cluedit"
#[allow(dead_code)]
pub fn sanitize_project_name(raw_name: &str) -> String {
    // Claude encodes paths by replacing / with - and prepending -
    // Split on - and take the last meaningful segment
    let segments: Vec<&str> = raw_name.split('-').filter(|s| !s.is_empty()).collect();

    if segments.is_empty() {
        return raw_name.to_string();
    }

    // The last segment is typically the project/directory name
    let last = segments.last().unwrap();

    // If the last segment is generic (like "work", "src", "dev"), try the last two
    let generic = ["work", "src", "dev", "home", "Users", "tmp", "var"];
    if segments.len() >= 2 && generic.contains(last) {
        return segments[segments.len() - 2..].join("-");
    }

    last.to_string()
}

// ============================================================================
// Text-only extraction (for conversational training mode)
// ============================================================================

use crate::models::{ContentBlock, ConversationEvent, MessageContent};

/// Extract ONLY text content from a conversation event, ignoring all
/// tool_use and tool_result blocks. Returns None if no text blocks exist.
/// This produces clean conversational data without tool noise.
pub fn extract_text_only(event: &ConversationEvent) -> Option<(String, String)> {
    let (message, is_meta, role) = match event {
        ConversationEvent::User {
            message, is_meta, ..
        } => (message, *is_meta, "user"),
        ConversationEvent::Assistant {
            message, is_meta, ..
        } => (message, *is_meta, "assistant"),
        _ => return None,
    };

    if is_meta {
        return None;
    }

    let text = match &message.content {
        MessageContent::Text(s) => s.trim().to_string(),
        MessageContent::Blocks(blocks) => {
            let texts: Vec<&str> = blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => {
                        let t = text.trim();
                        if t.is_empty() {
                            None
                        } else {
                            Some(t)
                        }
                    }
                    _ => None, // Skip ToolUse, ToolResult, Other
                })
                .collect();
            if texts.is_empty() {
                return None;
            }
            texts.join("\n\n")
        }
    };

    if text.is_empty() {
        return None;
    }

    Some((role.to_string(), text))
}

// ============================================================================
// Token estimation & chunking
// ============================================================================

/// Estimate token count for a string.
/// Uses char count (not byte count) with 20% safety margin to handle
/// code-heavy content where tokens are shorter than 4 chars.
pub fn estimate_tokens(text: &str) -> usize {
    let by_chars = text.chars().count().div_ceil(4);
    // Add 20% headroom for code-heavy content
    by_chars + by_chars / 5
}

/// Default maximum tokens per training example.
pub const DEFAULT_MAX_TOKENS: usize = 16384;

/// Split turns into chunks that each fit within a token budget.
/// Each chunk will be emitted as a separate JSONL training example.
/// Keeps user-assistant pairs together (never splits mid-pair).
pub fn chunk_turns(turns: &[Turn], max_tokens: usize, system_tokens: usize) -> Vec<Vec<&Turn>> {
    if turns.is_empty() {
        return vec![];
    }

    let mut chunks: Vec<Vec<&Turn>> = Vec::new();
    let mut current_chunk: Vec<&Turn> = Vec::new();
    let mut current_tokens = system_tokens;

    for turn in turns {
        let turn_tokens = estimate_tokens(&turn.content) + 10; // role overhead

        if !current_chunk.is_empty() && current_tokens + turn_tokens > max_tokens {
            // This turn would exceed budget — start a new chunk
            // But ensure the current chunk has at least one user and one assistant
            if current_chunk.iter().any(|t| t.role == "user")
                && current_chunk.iter().any(|t| t.role == "assistant")
            {
                chunks.push(current_chunk);
                current_chunk = Vec::new();
                current_tokens = system_tokens;
            }
            // If the chunk doesn't have both roles yet, keep adding
        }

        current_tokens += turn_tokens;
        current_chunk.push(turn);
    }

    // Don't forget the last chunk
    if !current_chunk.is_empty()
        && current_chunk.iter().any(|t| t.role == "user")
        && current_chunk.iter().any(|t| t.role == "assistant")
    {
        chunks.push(current_chunk);
    }

    chunks
}

// ============================================================================
// OpenAI Tool Schema Generation (for ChatMLTools format)
// ============================================================================

/// Generate OpenAI function-calling tool definitions for known Claude Code tools.
pub fn generate_tool_schemas(tool_names: &[String]) -> Vec<serde_json::Value> {
    tool_names
        .iter()
        .filter_map(|name| tool_schema(name))
        .collect()
}

fn tool_schema(name: &str) -> Option<serde_json::Value> {
    let schema = match name {
        "Bash" => serde_json::json!({
            "type": "function",
            "function": {
                "name": "Bash",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {"type": "string"},
                        "timeout": {"type": "number"}
                    },
                    "required": ["command"]
                }
            }
        }),
        "Read" => serde_json::json!({
            "type": "function",
            "function": {
                "name": "Read",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "offset": {"type": "number"},
                        "limit": {"type": "number"}
                    },
                    "required": ["file_path"]
                }
            }
        }),
        "Write" => serde_json::json!({
            "type": "function",
            "function": {
                "name": "Write",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "content": {"type": "string"}
                    },
                    "required": ["file_path", "content"]
                }
            }
        }),
        "Edit" => serde_json::json!({
            "type": "function",
            "function": {
                "name": "Edit",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "old_string": {"type": "string"},
                        "new_string": {"type": "string"},
                        "replace_all": {"type": "boolean"}
                    },
                    "required": ["file_path", "old_string", "new_string"]
                }
            }
        }),
        "Grep" => serde_json::json!({
            "type": "function",
            "function": {
                "name": "Grep",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string"},
                        "path": {"type": "string"},
                        "glob": {"type": "string"}
                    },
                    "required": ["pattern"]
                }
            }
        }),
        "Glob" => serde_json::json!({
            "type": "function",
            "function": {
                "name": "Glob",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string"},
                        "path": {"type": "string"}
                    },
                    "required": ["pattern"]
                }
            }
        }),
        "Agent" => serde_json::json!({
            "type": "function",
            "function": {
                "name": "Agent",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "prompt": {"type": "string"},
                        "description": {"type": "string"},
                        "subagent_type": {"type": "string"}
                    },
                    "required": ["description", "prompt"]
                }
            }
        }),
        _ => {
            // Generic schema for unknown tools
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": name,
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": true
                    }
                }
            })
        }
    };
    Some(schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_skip_command() {
        let cmd = r#"<command-name>/clear</command-name>
            <command-message>clear</command-message>
            <command-args></command-args>"#;
        assert!(should_skip_message(cmd));
    }

    #[test]
    fn test_should_skip_stdout_only() {
        let stdout = "<local-command-stdout>Set effort level to max</local-command-stdout>";
        assert!(should_skip_message(stdout));
    }

    #[test]
    fn test_should_not_skip_real_content() {
        let msg = "Please implement the authentication feature";
        assert!(!should_skip_message(msg));
    }

    #[test]
    fn test_sanitize_strips_tags() {
        let text = "Hello <system-reminder>ignored</system-reminder> world";
        let result = sanitize_for_training(text);
        assert_eq!(result, "Hello  world");
    }

    #[test]
    fn test_sanitize_project_name() {
        assert_eq!(
            sanitize_project_name("-Users-nickpaterno-work-cluedit"),
            "cluedit"
        );
        assert_eq!(sanitize_project_name("my-project"), "project");
        assert_eq!(sanitize_project_name("simple"), "simple");
    }

    #[test]
    fn test_merge_consecutive() {
        let turns = vec![
            Turn {
                role: "user".into(),
                content: "hello".into(),
            },
            Turn {
                role: "user".into(),
                content: "world".into(),
            },
            Turn {
                role: "assistant".into(),
                content: "hi".into(),
            },
        ];
        let merged = merge_consecutive_turns(turns);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].content, "hello\n\nworld");
        assert_eq!(merged[1].content, "hi");
    }

    #[test]
    fn test_estimate_tokens() {
        // 11 chars → ceil(11/4) = 3, +20% = 3
        assert!(estimate_tokens("hello world") >= 3);
        assert_eq!(estimate_tokens(""), 0);
        // Should be > 0 for any non-empty string
        assert!(estimate_tokens("abcd") >= 1);
        // Longer strings get safety margin
        let long = "a".repeat(100);
        assert!(estimate_tokens(&long) > 25); // base 25 + margin
    }

    #[test]
    fn test_chunk_turns_fits_in_one() {
        let turns = vec![
            Turn {
                role: "user".into(),
                content: "hi".into(),
            },
            Turn {
                role: "assistant".into(),
                content: "hello".into(),
            },
        ];
        let chunks = chunk_turns(&turns, 1000, 50);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_chunk_turns_splits() {
        let turns = vec![
            Turn {
                role: "user".into(),
                content: "a".repeat(200),
            },
            Turn {
                role: "assistant".into(),
                content: "b".repeat(200),
            },
            Turn {
                role: "user".into(),
                content: "c".repeat(200),
            },
            Turn {
                role: "assistant".into(),
                content: "d".repeat(200),
            },
        ];
        // max 150 tokens, each turn ≈ 60 tokens. Should split into 2 chunks.
        let chunks = chunk_turns(&turns, 150, 10);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_low_value_assistant() {
        assert!(is_low_value_assistant("Let me read the file."));
        assert!(is_low_value_assistant(
            "Now let me check the configuration."
        ));
        assert!(!is_low_value_assistant("The function `parse_config` takes a path parameter and returns a Config struct. Here's how it works:"));
        assert!(!is_low_value_assistant("```rust\nfn main() {}\n```"));
    }
}
