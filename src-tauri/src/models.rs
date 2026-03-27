use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================================
// Claude Code JSONL Event Models
// ============================================================================

/// Content block within a Claude API message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Option<serde_json::Value>,
        #[serde(default)]
        is_error: bool,
    },
    #[serde(untagged)]
    Other(serde_json::Value),
}

/// Message content can be a plain string or an array of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    /// Extract all plain text from the content (regardless of format)
    pub fn extract_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
        }
    }

    /// Get tool use blocks (assistant messages only)
    pub fn tool_uses(&self) -> Vec<&ContentBlock> {
        match self {
            MessageContent::Text(_) => vec![],
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter(|b| matches!(b, ContentBlock::ToolUse { .. }))
                .collect(),
        }
    }
}

/// The nested message object within user/assistant events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: MessageContent,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
    #[serde(default)]
    pub usage: Option<serde_json::Value>,
}

/// A single line/event in a Claude Code JSONL conversation file.
///
/// The top-level `type` field discriminates event kinds:
/// - "user" / "assistant" — chat messages (contain a nested `message` object)
/// - "progress"           — tool execution / hook progress
/// - "system"             — system metadata (turn_duration, compact_boundary, …)
/// - "file-history-snapshot" — file backup snapshots
/// - "summary"            — conversation summary/title
/// - "queue-operation"    — queued user prompts
/// - "last-prompt"        — last prompt marker
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConversationEvent {
    #[serde(rename = "user")]
    User {
        message: ApiMessage,
        uuid: Option<String>,
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
        timestamp: Option<String>,
        #[serde(rename = "isMeta")]
        #[serde(default)]
        is_meta: bool,
        #[serde(rename = "isSidechain")]
        #[serde(default)]
        is_sidechain: bool,
        cwd: Option<String>,
        version: Option<String>,
        #[serde(rename = "gitBranch")]
        git_branch: Option<String>,
        slug: Option<String>,
        #[serde(rename = "logicalParentUuid")]
        logical_parent_uuid: Option<String>,
        #[serde(rename = "promptId")]
        #[serde(default)]
        prompt_id: Option<String>,
        #[serde(rename = "permissionMode")]
        #[serde(default)]
        permission_mode: Option<String>,
        #[serde(rename = "isCompactSummary")]
        #[serde(default)]
        is_compact_summary: bool,
        #[serde(rename = "agentId")]
        #[serde(default)]
        agent_id: Option<String>,
        #[serde(rename = "sourceToolUseId")]
        #[serde(default)]
        source_tool_use_id: Option<String>,
        #[serde(rename = "sourceToolAssistantUuid")]
        #[serde(default)]
        source_tool_assistant_uuid: Option<String>,
        #[serde(rename = "toolUseResult")]
        #[serde(default)]
        tool_use_result: Option<serde_json::Value>,
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    #[serde(rename = "assistant")]
    Assistant {
        message: ApiMessage,
        uuid: Option<String>,
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
        timestamp: Option<String>,
        #[serde(rename = "isMeta")]
        #[serde(default)]
        is_meta: bool,
        #[serde(rename = "isSidechain")]
        #[serde(default)]
        is_sidechain: bool,
        #[serde(rename = "requestId")]
        request_id: Option<String>,
        cwd: Option<String>,
        version: Option<String>,
        slug: Option<String>,
        #[serde(rename = "agentId")]
        #[serde(default)]
        agent_id: Option<String>,
        #[serde(rename = "sourceToolUseId")]
        #[serde(default)]
        source_tool_use_id: Option<String>,
        #[serde(rename = "sourceToolAssistantUuid")]
        #[serde(default)]
        source_tool_assistant_uuid: Option<String>,
        #[serde(rename = "toolUseResult")]
        #[serde(default)]
        tool_use_result: Option<serde_json::Value>,
        #[serde(rename = "isApiErrorMessage")]
        #[serde(default)]
        is_api_error_message: bool,
        #[serde(default)]
        error: Option<serde_json::Value>,
        #[serde(rename = "thinkingMetadata")]
        #[serde(default)]
        thinking_metadata: Option<serde_json::Value>,
        #[serde(rename = "logicalParentUuid")]
        #[serde(default)]
        logical_parent_uuid: Option<String>,
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    #[serde(rename = "progress")]
    Progress {
        data: Option<serde_json::Value>,
        #[serde(rename = "parentToolUseID")]
        parent_tool_use_id: Option<String>,
        #[serde(rename = "toolUseID")]
        tool_use_id: Option<String>,
        uuid: Option<String>,
        timestamp: Option<String>,
        #[serde(rename = "agentId")]
        #[serde(default)]
        agent_id: Option<String>,
        #[serde(rename = "sessionId")]
        #[serde(default)]
        session_id: Option<String>,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    #[serde(rename = "system")]
    System {
        subtype: Option<String>,
        uuid: Option<String>,
        timestamp: Option<String>,
        #[serde(rename = "isMeta")]
        #[serde(default)]
        is_meta: bool,
        #[serde(default)]
        content: Option<serde_json::Value>,
        #[serde(rename = "durationMs")]
        #[serde(default)]
        duration_ms: Option<f64>,
        #[serde(rename = "compactMetadata")]
        #[serde(default)]
        compact_metadata: Option<serde_json::Value>,
        #[serde(rename = "logicalParentUuid")]
        #[serde(default)]
        logical_parent_uuid: Option<String>,
        #[serde(default)]
        level: Option<String>,
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot {
        #[serde(rename = "messageId")]
        message_id: String,
        snapshot: serde_json::Value,
        #[serde(rename = "isSnapshotUpdate")]
        #[serde(default)]
        is_snapshot_update: bool,
    },
    #[serde(rename = "summary")]
    Summary {
        summary: Option<String>,
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    #[serde(rename = "queue-operation")]
    QueueOperation {
        operation: Option<String>,
        content: Option<String>,
        timestamp: Option<String>,
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    #[serde(rename = "last-prompt")]
    LastPrompt {
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    #[serde(rename = "pr-link")]
    PrLink {
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
        #[serde(rename = "prNumber")]
        pr_number: Option<u32>,
        #[serde(rename = "prUrl")]
        pr_url: Option<String>,
        #[serde(rename = "prRepository")]
        pr_repository: Option<String>,
        timestamp: Option<String>,
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    /// Catch-all for unknown event types
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

impl ConversationEvent {
    /// Get the event type as a display string
    pub fn event_type(&self) -> &str {
        match self {
            Self::User { .. } => "user",
            Self::Assistant { .. } => "assistant",
            Self::Progress { .. } => "progress",
            Self::System { .. } => "system",
            Self::FileHistorySnapshot { .. } => "file-history-snapshot",
            Self::Summary { .. } => "summary",
            Self::QueueOperation { .. } => "queue-operation",
            Self::LastPrompt { .. } => "last-prompt",
            Self::PrLink { .. } => "pr-link",
            Self::Unknown(_) => "unknown",
        }
    }

    /// Check if this is a user or assistant message (not meta)
    pub fn is_chat_message(&self) -> bool {
        match self {
            Self::User { is_meta, .. } => !is_meta,
            Self::Assistant { is_meta, .. } => !is_meta,
            _ => false,
        }
    }

    /// Get the message content text if this is a chat message
    pub fn message_text(&self) -> Option<String> {
        match self {
            Self::User {
                message, is_meta, ..
            } if !is_meta => Some(message.content.extract_text()),
            Self::Assistant {
                message, is_meta, ..
            } if !is_meta => Some(message.content.extract_text()),
            _ => None,
        }
    }

    /// Get the role of this event ("user", "assistant", or None)
    pub fn role(&self) -> Option<&str> {
        match self {
            Self::User { .. } => Some("user"),
            Self::Assistant { .. } => Some("assistant"),
            _ => None,
        }
    }

    /// Get the UUID of this event
    pub fn uuid(&self) -> Option<&str> {
        match self {
            Self::User { uuid, .. } => uuid.as_deref(),
            Self::Assistant { uuid, .. } => uuid.as_deref(),
            Self::Progress { uuid, .. } => uuid.as_deref(),
            Self::System { uuid, .. } => uuid.as_deref(),
            _ => None,
        }
    }

    /// Get the timestamp string of this event
    pub fn timestamp(&self) -> Option<&str> {
        match self {
            Self::User { timestamp, .. } => timestamp.as_deref(),
            Self::Assistant { timestamp, .. } => timestamp.as_deref(),
            Self::Progress { timestamp, .. } => timestamp.as_deref(),
            Self::System { timestamp, .. } => timestamp.as_deref(),
            Self::QueueOperation { timestamp, .. } => timestamp.as_deref(),
            _ => None,
        }
    }

    /// Get the logical parent UUID (for continuation detection)
    pub fn logical_parent_uuid(&self) -> Option<&str> {
        match self {
            Self::User {
                logical_parent_uuid,
                ..
            } => logical_parent_uuid.as_deref(),
            _ => None,
        }
    }
}

// ============================================================================
// Application Models
// ============================================================================

/// Metadata about a conversation file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMetadata {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub created: i64,
    pub modified: i64,
    pub event_count: usize,
    pub project: Option<String>,
    pub first_message: Option<String>,

    // Enhanced metadata
    pub title: Option<String>,
    pub summary: Option<String>,
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

/// Full conversation with all events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub metadata: ConversationMetadata,
    pub events: Vec<ConversationEvent>,
}

/// Search result with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub conversation_id: String,
    pub file_path: String,
    pub matches: Vec<SearchMatch>,
    pub total_matches: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub line_number: usize,
    pub content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

/// Conversation data provider
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Provider {
    Claude,
    Codex,
}

/// Provider availability info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub provider: Provider,
    pub available: bool,
}

/// Project directory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: PathBuf,
    pub conversation_count: usize,
    #[serde(default = "default_provider")]
    pub provider: Provider,
}

fn default_provider() -> Provider {
    Provider::Claude
}

/// Redaction configuration for training data export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactConfig {
    #[serde(default = "default_true")]
    pub redact_api_keys: bool,
    #[serde(default = "default_true")]
    pub redact_home_paths: bool,
    #[serde(default)]
    pub redact_emails: bool,
    #[serde(default)]
    pub redact_ip_addresses: bool,
    #[serde(default)]
    pub custom_rules: Vec<RedactRule>,
}

fn default_true() -> bool {
    true
}

impl Default for RedactConfig {
    fn default() -> Self {
        Self {
            redact_api_keys: true,
            redact_home_paths: true,
            redact_emails: false,
            redact_ip_addresses: false,
            custom_rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactRule {
    pub pattern: String,
    pub replacement: String,
    #[serde(default)]
    pub is_regex: bool,
}

/// Export format options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    JsonPretty,
    Markdown,
    Text,
    /// OpenAI ChatML / messages array format for fine-tuning
    ChatML,
    /// ShareGPT format (conversations array with from/value pairs)
    ShareGPT,
    /// Alpaca instruction format (instruction/input/output triplets)
    Alpaca,
    /// OpenAI fine-tuning with structured tool_calls and role:"tool" messages
    ChatMLTools,
}

// ============================================================================
// Backup & Branch Models
// ============================================================================

/// Metadata about a single conversation backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub conversation_id: String,
    pub original_file_path: String,
    pub backup_file_path: String,
    pub label: String,
    pub created_at: String,
    pub event_count: usize,
    pub truncated_at_event: Option<usize>,
    pub size_bytes: u64,
    pub auto_backup: bool,
}

/// Result of a conversation branch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchResult {
    pub new_file_path: String,
    pub new_conversation_id: String,
    pub event_count: usize,
    pub ids_remapped: usize,
}

/// Result of a bulk export operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportAllResult {
    pub conversations_exported: usize,
    pub conversations_skipped: usize,
    pub output_path: String,
}
