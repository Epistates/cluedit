use crate::error::Result;
use crate::models::ConversationEvent;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Analyzes conversations to extract rich metadata
pub struct ConversationAnalyzer;

#[derive(Debug)]
pub struct AnalysisResult {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub event_count: usize,
    pub user_message_count: usize,
    pub total_message_count: usize,
    pub artifact_count: usize,
    pub artifacts: Vec<String>,
    pub is_continuation: bool,
    pub continued_from_id: Option<String>,
    pub has_compaction: bool,
    pub last_user_message: Option<String>,
    pub topics: Vec<String>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub session_count: usize,
    pub tool_use_count: usize,
    pub tool_names: Vec<String>,
}

/// Internal result struct for parse_conversation_file
struct ParseResult {
    artifacts: Vec<String>,
    has_compaction: bool,
    user_messages: Vec<String>,
    total_message_count: usize,
    event_count: usize,
    parent_uuid: Option<String>,
    explicit_title: Option<String>,
    total_input_tokens: u64,
    total_output_tokens: u64,
    session_count: usize,
    tool_use_count: usize,
    tool_names: Vec<String>,
}

impl ConversationAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Analyze a conversation file to extract rich metadata
    pub fn analyze(
        &self,
        conversation_path: &Path,
        _conversation_id: &str,
    ) -> Result<AnalysisResult> {
        let parsed = self.parse_conversation_file(conversation_path)?;

        let title = if !parsed.user_messages.is_empty() {
            self.generate_title(&parsed.user_messages)
        } else {
            parsed.explicit_title
        };

        let summary = self.generate_summary(&parsed.user_messages);
        let is_continuation = parsed.parent_uuid.is_some();
        let continued_from_id = parsed.parent_uuid;
        let last_user_message = parsed.user_messages.last().cloned();
        let topics = self.extract_topics(&parsed.user_messages);

        Ok(AnalysisResult {
            title,
            summary,
            event_count: parsed.event_count,
            user_message_count: parsed.user_messages.len(),
            total_message_count: parsed.total_message_count,
            artifact_count: parsed.artifacts.len(),
            artifacts: parsed.artifacts,
            is_continuation,
            continued_from_id,
            has_compaction: parsed.has_compaction,
            last_user_message,
            topics,
            total_input_tokens: parsed.total_input_tokens,
            total_output_tokens: parsed.total_output_tokens,
            session_count: parsed.session_count,
            tool_use_count: parsed.tool_use_count,
            tool_names: parsed.tool_names,
        })
    }

    /// Parse conversation file using typed ConversationEvent models
    fn parse_conversation_file(&self, conversation_path: &Path) -> Result<ParseResult> {
        let file = File::open(conversation_path)?;
        let reader = BufReader::new(file);

        let mut artifacts: HashSet<String> = HashSet::new();
        let mut has_compaction = false;
        let mut user_messages: Vec<String> = Vec::new();
        let mut total_message_count: usize = 0;
        let mut event_count: usize = 0;
        let mut parent_uuid: Option<String> = None;
        let mut explicit_title: Option<String> = None;
        let mut is_first_event = true;
        let mut session_ids: HashSet<String> = HashSet::new();
        let mut tool_use_count: usize = 0;
        let mut tool_name_set: HashSet<String> = HashSet::new();
        let mut total_input_tokens: u64 = 0;
        let mut total_output_tokens: u64 = 0;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            event_count += 1;

            match serde_json::from_str::<ConversationEvent>(&line) {
                Ok(event) => {
                    // Check for logical parent UUID on first user event (continuation detection)
                    if is_first_event {
                        if let Some(lpu) = event.logical_parent_uuid() {
                            if !lpu.is_empty() {
                                parent_uuid = Some(lpu.to_string());
                            }
                        }
                        is_first_event = false;
                    }

                    match &event {
                        ConversationEvent::User {
                            message,
                            is_meta,
                            session_id,
                            ..
                        } => {
                            if let Some(sid) = session_id {
                                session_ids.insert(sid.clone());
                            }
                            if !is_meta {
                                total_message_count += 1;
                                let text = message.content.extract_text();
                                if !text.trim().is_empty() && !text.starts_with("Caveat:") {
                                    user_messages.push(text);
                                }
                            }
                        }
                        ConversationEvent::Assistant {
                            message,
                            is_meta,
                            session_id,
                            ..
                        } => {
                            if let Some(sid) = session_id {
                                session_ids.insert(sid.clone());
                            }
                            if !is_meta {
                                total_message_count += 1;
                                // Extract token usage
                                if let Some(usage) = &message.usage {
                                    if let Some(input) =
                                        usage.get("input_tokens").and_then(|v| v.as_u64())
                                    {
                                        total_input_tokens += input;
                                    }
                                    if let Some(output) =
                                        usage.get("output_tokens").and_then(|v| v.as_u64())
                                    {
                                        total_output_tokens += output;
                                    }
                                }
                                // Count tool uses
                                for block in message.content.tool_uses() {
                                    tool_use_count += 1;
                                    if let crate::models::ContentBlock::ToolUse { name, .. } = block
                                    {
                                        tool_name_set.insert(name.clone());
                                    }
                                }
                            }
                        }
                        ConversationEvent::System { subtype, .. } => {
                            if subtype.as_deref() == Some("compact_boundary") {
                                has_compaction = true;
                            }
                        }
                        ConversationEvent::FileHistorySnapshot { snapshot, .. } => {
                            if let Some(backups) = snapshot
                                .get("trackedFileBackups")
                                .and_then(|b| b.as_object())
                            {
                                for (file_path, _) in backups {
                                    artifacts.insert(file_path.clone());
                                }
                            }
                        }
                        ConversationEvent::Summary {
                            summary: Some(s), ..
                        } => {
                            explicit_title = Some(s.clone());
                        }
                        _ => {}
                    }
                }
                Err(_) => {
                    // Fallback: try raw JSON for unrecognized formats
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                        if is_first_event {
                            if let Some(lpu) =
                                value.get("logicalParentUuid").and_then(|p| p.as_str())
                            {
                                if !lpu.is_empty() {
                                    parent_uuid = Some(lpu.to_string());
                                }
                            }
                            is_first_event = false;
                        }
                    }
                }
            }
        }

        let session_count = session_ids.len();
        let mut tool_names: Vec<String> = tool_name_set.into_iter().collect();
        tool_names.sort();

        Ok(ParseResult {
            artifacts: artifacts.into_iter().collect(),
            has_compaction,
            user_messages,
            total_message_count,
            event_count,
            parent_uuid,
            explicit_title,
            total_input_tokens,
            total_output_tokens,
            session_count,
            tool_use_count,
            tool_names,
        })
    }

    /// Generate a smart title from user messages
    fn generate_title(&self, messages: &[String]) -> Option<String> {
        if messages.is_empty() {
            return None;
        }

        for msg in messages.iter().take(5) {
            if msg.len() < 10 {
                continue;
            }
            if self.is_generic_message(msg) {
                continue;
            }
            let title = self.extract_first_sentence(msg);
            return Some(title);
        }

        let first = &messages[0];
        Some(truncate_utf8(first, 100))
    }

    /// Generate a summary from first few messages
    fn generate_summary(&self, messages: &[String]) -> Option<String> {
        if messages.is_empty() {
            return None;
        }

        let summary: String = messages
            .iter()
            .take(3)
            .filter(|m| !self.is_generic_message(m))
            .take(2)
            .map(|m| truncate_utf8(m, 150))
            .collect::<Vec<_>>()
            .join(" \u{2192} ");

        if summary.is_empty() {
            None
        } else {
            Some(summary)
        }
    }

    /// Extract topics/keywords from messages
    fn extract_topics(&self, messages: &[String]) -> Vec<String> {
        let mut topics: HashSet<String> = HashSet::new();

        let keywords = [
            "tauri",
            "rust",
            "svelte",
            "typescript",
            "python",
            "api",
            "backend",
            "frontend",
            "database",
            "test",
            "bug",
            "fix",
            "refactor",
            "feature",
            "performance",
            "security",
            "ui",
            "ux",
            "design",
            "deploy",
            "error",
            "issue",
            "implement",
            "optimize",
            "review",
        ];

        for msg in messages {
            let lower = msg.to_lowercase();
            for keyword in &keywords {
                if lower.contains(keyword) {
                    topics.insert(keyword.to_string());
                }
            }
        }

        topics.into_iter().take(5).collect()
    }

    /// Check if message is generic/unhelpful
    fn is_generic_message(&self, msg: &str) -> bool {
        let generic_patterns = [
            "proceed",
            "continue",
            "ok",
            "yes",
            "no",
            "thanks",
            "thank you",
            "/clear",
            "/status",
        ];

        let lower = msg.to_lowercase();
        generic_patterns
            .iter()
            .any(|p| lower.contains(p) && lower.len() < 30)
    }

    /// Extract first sentence from text
    fn extract_first_sentence(&self, text: &str) -> String {
        let sentence_end = text.find(['.', '?', '!']).unwrap_or(text.len());

        let limit = sentence_end.min(150);
        let sentence = truncate_at_char_boundary(text, limit);
        truncate_utf8(sentence, 100)
    }
}

/// Truncate text to max_len characters safely (respecting UTF-8 char boundaries)
pub fn truncate_utf8(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

/// Get a string slice up to `max_bytes` without splitting a UTF-8 character
fn truncate_at_char_boundary(text: &str, max_bytes: usize) -> &str {
    if max_bytes >= text.len() {
        return text;
    }
    // Walk backwards to find a valid char boundary
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
