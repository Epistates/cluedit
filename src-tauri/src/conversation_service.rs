use crate::conversation_analyzer::ConversationAnalyzer;
use crate::error::{ClueditError, Result};
use crate::models::*;
use crate::title_cache::TitleCache;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;
use walkdir::WalkDir;

/// Cached metadata with modification time for invalidation
#[derive(Clone)]
struct CachedMetadata {
    metadata: ConversationMetadata,
    mtime: SystemTime,
}

/// Service for managing Claude conversation files.
/// Designed to be long-lived as Tauri managed state so caches persist.
pub struct ConversationService {
    claude_dir: PathBuf,
    analyzer: ConversationAnalyzer,
    cache: Mutex<HashMap<PathBuf, CachedMetadata>>,
    title_cache: Mutex<TitleCache>,
}

impl ConversationService {
    /// Canonicalize `path` and verify it resides within `self.claude_dir`.
    fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        let canonical = path.canonicalize().map_err(|_| {
            ClueditError::InvalidPath(format!("Path not found: {}", path.display()))
        })?;
        let claude_canonical = self.claude_dir.canonicalize().map_err(|_| {
            ClueditError::InvalidPath("Claude directory not accessible".to_string())
        })?;
        if !canonical.starts_with(&claude_canonical) {
            return Err(ClueditError::InvalidPath(format!(
                "Path {} is outside the Claude directory",
                path.display()
            )));
        }
        Ok(canonical)
    }

    pub fn new(data_dir: &Path) -> Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| {
            ClueditError::InvalidPath("Home directory not found".to_string())
        })?;
        let claude_dir = home.join(".claude");

        if !claude_dir.exists() {
            return Err(ClueditError::NotFound(format!(
                "Claude directory not found at {}",
                claude_dir.display()
            )));
        }

        let analyzer = ConversationAnalyzer::new();
        let cache = Mutex::new(HashMap::new());
        let title_cache = Mutex::new(TitleCache::new(data_dir)?);

        Ok(Self {
            claude_dir,
            analyzer,
            cache,
            title_cache,
        })
    }

    /// List all project directories
    pub fn list_projects(&self) -> Result<Vec<ProjectInfo>> {
        let projects_dir = self.claude_dir.join("projects");
        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut projects = Vec::new();

        for entry in fs::read_dir(&projects_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let conversation_count = self.count_conversations(&path)?;

                projects.push(ProjectInfo {
                    name,
                    path,
                    conversation_count,
                });
            }
        }

        projects.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(projects)
    }

    /// Count conversations in a directory
    fn count_conversations(&self, dir: &Path) -> Result<usize> {
        let count = WalkDir::new(dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
            .count();
        Ok(count)
    }

    /// List all conversations in a project.
    /// Uses lightweight metadata (fast) with caching for instant re-loads.
    pub fn list_conversations(&self, project_path: &str) -> Result<Vec<ConversationMetadata>> {
        let raw_path = PathBuf::from(project_path);
        let path = self.validate_path(&raw_path)?;

        let mut conversations = Vec::new();

        for entry in WalkDir::new(&path)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();
            if entry_path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                // Try cache first
                let cached = {
                    let cache = self.cache.lock().unwrap();
                    if let Some(cached) = cache.get(entry_path) {
                        if let Ok(current_metadata) = fs::metadata(entry_path) {
                            if let Ok(current_mtime) = current_metadata.modified() {
                                if current_mtime == cached.mtime {
                                    Some(cached.metadata.clone())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                let metadata = if let Some(cached_metadata) = cached {
                    cached_metadata
                } else {
                    let meta = self.get_lightweight_metadata(entry_path)?;
                    if let Ok(file_metadata) = fs::metadata(entry_path) {
                        if let Ok(mtime) = file_metadata.modified() {
                            let mut cache = self.cache.lock().unwrap();
                            cache.insert(
                                entry_path.to_path_buf(),
                                CachedMetadata {
                                    metadata: meta.clone(),
                                    mtime,
                                },
                            );
                        }
                    }
                    meta
                };

                conversations.push(metadata);
            }
        }

        conversations.sort_by(|a, b| b.modified.cmp(&a.modified));
        Ok(conversations)
    }

    /// Get lightweight metadata (filesystem + quick line counting, no full parse)
    fn get_lightweight_metadata(&self, file_path: &Path) -> Result<ConversationMetadata> {
        let metadata = fs::metadata(file_path)?;
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let id = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let created = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let project = file_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        // Quick counts without full JSON parsing
        let (event_count, total_message_count_quick) = fs::File::open(file_path)
            .and_then(|file| {
                let reader = BufReader::new(file);
                let mut events = 0;
                let mut messages = 0;

                for line in reader.lines() {
                    events += 1;
                    if let Ok(line) = line {
                        if (line.contains("\"type\":\"user\"")
                            || line.contains("\"type\":\"assistant\""))
                            && !line.contains("\"isMeta\":true")
                        {
                            messages += 1;
                        }
                    }
                }

                Ok((events, messages))
            })
            .unwrap_or((0, 0));

        // Check title cache
        let (title, summary) = if let Ok(mtime) = metadata.modified() {
            let title_cache = self.title_cache.lock().unwrap();
            if let Some(cached) = title_cache.get(&id, mtime) {
                (cached.title, cached.summary)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        Ok(ConversationMetadata {
            id,
            file_path: file_path.to_string_lossy().to_string(),
            file_name,
            size_bytes: metadata.len(),
            created,
            modified,
            event_count,
            project,
            first_message: None,
            title,
            summary,
            user_message_count: 0,
            total_message_count: total_message_count_quick,
            artifact_count: 0,
            artifacts: Vec::new(),
            is_continuation: false,
            continued_from_id: None,
            has_compaction: false,
            last_user_message: None,
            topics: Vec::new(),
            total_input_tokens: 0,
            total_output_tokens: 0,
            session_count: 0,
            tool_use_count: 0,
            tool_names: Vec::new(),
        })
    }

    /// Get metadata for a single conversation file (with full analysis).
    /// Caches the result for subsequent calls.
    pub fn get_conversation_metadata(&self, file_path: &Path) -> Result<ConversationMetadata> {
        let file_path = &self.validate_path(file_path)?;
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(file_path) {
                if let Ok(current_metadata) = fs::metadata(file_path) {
                    if let Ok(current_mtime) = current_metadata.modified() {
                        if current_mtime == cached.mtime
                            && (cached.metadata.title.is_some()
                                || cached.metadata.total_message_count > 0)
                        {
                            return Ok(cached.metadata.clone());
                        }
                    }
                }
            }
        }

        let metadata = fs::metadata(file_path)?;
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let id = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let analysis = self
            .analyzer
            .analyze(file_path, &id)
            .unwrap_or_else(|_| crate::conversation_analyzer::AnalysisResult {
                title: None,
                summary: None,
                event_count: 0,
                user_message_count: 0,
                total_message_count: 0,
                artifact_count: 0,
                artifacts: Vec::new(),
                is_continuation: false,
                continued_from_id: None,
                has_compaction: false,
                last_user_message: None,
                topics: Vec::new(),
                total_input_tokens: 0,
                total_output_tokens: 0,
                session_count: 0,
                tool_use_count: 0,
                tool_names: Vec::new(),
            });

        let created = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let project = file_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        let conversation_metadata = ConversationMetadata {
            id: id.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            file_name,
            size_bytes: metadata.len(),
            created,
            modified,
            event_count: analysis.event_count,
            project,
            first_message: analysis.last_user_message.clone(),
            title: analysis.title,
            summary: analysis.summary,
            user_message_count: analysis.user_message_count,
            total_message_count: analysis.total_message_count,
            artifact_count: analysis.artifact_count,
            artifacts: analysis.artifacts,
            is_continuation: analysis.is_continuation,
            continued_from_id: analysis.continued_from_id,
            has_compaction: analysis.has_compaction,
            last_user_message: analysis.last_user_message,
            topics: analysis.topics,
            total_input_tokens: analysis.total_input_tokens,
            total_output_tokens: analysis.total_output_tokens,
            session_count: analysis.session_count,
            tool_use_count: analysis.tool_use_count,
            tool_names: analysis.tool_names,
        };

        // Cache the fully analyzed metadata
        if let Ok(mtime) = metadata.modified() {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(
                file_path.to_path_buf(),
                CachedMetadata {
                    metadata: conversation_metadata.clone(),
                    mtime,
                },
            );

            // Also save to persistent title cache
            let mut title_cache = self.title_cache.lock().unwrap();
            title_cache.set(
                id,
                conversation_metadata.title.clone(),
                conversation_metadata.summary.clone(),
                conversation_metadata.total_message_count,
                mtime,
            );
            if let Err(e) = title_cache.save() {
                log::warn!("Failed to save title cache: {}", e);
            }
        }

        Ok(conversation_metadata)
    }

    /// Read full conversation with typed events
    pub fn read_conversation(&self, file_path: &str) -> Result<Conversation> {
        let raw_path = PathBuf::from(file_path);
        let path = self.validate_path(&raw_path)?;
        // get_conversation_metadata also validates, but we pass the already-canonical path
        let metadata = self.get_conversation_metadata(&path)?;

        let file = fs::File::open(&path)?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                match serde_json::from_str::<ConversationEvent>(&line) {
                    Ok(event) => events.push(event),
                    Err(_) => {
                        // Fallback to raw JSON for completely unrecognized formats
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                            events.push(ConversationEvent::Unknown(value));
                        }
                    }
                }
            }
        }

        Ok(Conversation { metadata, events })
    }

    /// Search conversations for a query
    pub fn search_conversations(
        &self,
        query: &str,
        project_paths: Vec<String>,
        case_sensitive: bool,
        use_regex: bool,
    ) -> Result<Vec<SearchResult>> {
        let safe_query = if use_regex {
            if query.len() > 500 {
                return Err(ClueditError::InvalidPath(
                    "Regex query too long (max 500 chars)".to_string(),
                ));
            }
            query.to_string()
        } else {
            regex::escape(query)
        };

        let regex = if case_sensitive {
            Regex::new(&safe_query)?
        } else {
            Regex::new(&format!("(?i){}", safe_query))?
        };

        let mut results = Vec::new();

        for project_path in project_paths {
            let raw_path = PathBuf::from(&project_path);
            let path = match self.validate_path(&raw_path) {
                Ok(p) => p,
                Err(_) => continue,
            };

            for entry in WalkDir::new(&path)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                if entry_path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    if let Ok(result) = self.search_file(entry_path, &regex) {
                        if !result.matches.is_empty() {
                            results.push(result);
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Search a single file
    fn search_file(&self, file_path: &Path, regex: &Regex) -> Result<SearchResult> {
        let file = fs::File::open(file_path)?;
        let reader = BufReader::new(file);

        let mut matches = Vec::new();
        let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

        for (i, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                let context_before = if i > 0 {
                    vec![lines[i - 1].clone()]
                } else {
                    vec![]
                };

                let context_after = if i < lines.len() - 1 {
                    vec![lines[i + 1].clone()]
                } else {
                    vec![]
                };

                matches.push(SearchMatch {
                    line_number: i + 1,
                    content: line.clone(),
                    context_before,
                    context_after,
                });
            }
        }

        let conversation_id = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let total_matches = matches.len();
        Ok(SearchResult {
            conversation_id,
            file_path: file_path.to_string_lossy().to_string(),
            matches,
            total_matches,
        })
    }

    /// Export conversation to different format
    pub fn export_conversation(
        &self,
        file_path: &str,
        format: ExportFormat,
    ) -> Result<String> {
        // validate_path is called inside read_conversation, but we validate
        // eagerly here so the error surface is at the right layer.
        let raw_path = PathBuf::from(file_path);
        self.validate_path(&raw_path)?;
        let conversation = self.read_conversation(file_path)?;

        match format {
            ExportFormat::Json => Ok(serde_json::to_string(&conversation)?),
            ExportFormat::JsonPretty => Ok(serde_json::to_string_pretty(&conversation)?),
            ExportFormat::Markdown => self.export_to_markdown(&conversation),
            ExportFormat::Text => self.export_to_text(&conversation),
            ExportFormat::ChatML => self.export_to_chatml(&conversation),
            ExportFormat::ShareGPT => self.export_to_sharegpt(&conversation),
            ExportFormat::Alpaca => self.export_to_alpaca(&conversation),
        }
    }

    fn export_to_markdown(&self, conversation: &Conversation) -> Result<String> {
        let mut output = String::new();
        let title = conversation
            .metadata
            .title
            .as_deref()
            .unwrap_or(&conversation.metadata.id);
        output.push_str(&format!("# {}\n\n", title));
        output.push_str(&format!("- **File**: {}\n", conversation.metadata.file_name));
        output.push_str(&format!(
            "- **Messages**: {}\n",
            conversation.metadata.total_message_count
        ));
        output.push_str(&format!(
            "- **Size**: {} bytes\n\n",
            conversation.metadata.size_bytes
        ));

        output.push_str("## Conversation\n\n");
        for event in &conversation.events {
            if let Some(text) = event.message_text() {
                let role = event.role().unwrap_or("unknown");
                let role_label = if role == "user" { "User" } else { "Assistant" };
                output.push_str(&format!("### {}\n\n", role_label));
                output.push_str(&text);
                output.push_str("\n\n---\n\n");
            }
        }

        Ok(output)
    }

    /// Find a conversation file that contains an event with the given UUID.
    /// Used for continuation chaining - finding the parent conversation.
    pub fn find_conversation_by_uuid(&self, target_uuid: &str) -> Result<Option<String>> {
        let projects_dir = self.claude_dir.join("projects");
        if !projects_dir.exists() {
            return Ok(None);
        }

        for project_entry in std::fs::read_dir(&projects_dir)? {
            let project_entry = project_entry?;
            let project_path = project_entry.path();
            if !project_path.is_dir() {
                continue;
            }

            for entry in WalkDir::new(&project_path)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }

                // Quick check: see if the UUID string appears anywhere in the file
                let content = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if !content.contains(target_uuid) {
                    continue;
                }

                // Verify by parsing - check if any event has this UUID
                let file = match std::fs::File::open(path) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let reader = BufReader::new(file);

                for line in reader.lines().filter_map(|l| l.ok()) {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<ConversationEvent>(&line) {
                        if event.uuid().map_or(false, |u| u == target_uuid) {
                            return Ok(Some(path.to_string_lossy().to_string()));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    fn export_to_text(&self, conversation: &Conversation) -> Result<String> {
        let mut output = String::new();
        let title = conversation
            .metadata
            .title
            .as_deref()
            .unwrap_or(&conversation.metadata.id);
        output.push_str(&format!("Conversation: {}\n", title));
        output.push_str(&format!("File: {}\n", conversation.metadata.file_name));
        output.push_str(&format!(
            "Messages: {}\n\n",
            conversation.metadata.total_message_count
        ));

        output.push_str(&"=".repeat(80));
        output.push_str("\n\n");

        for event in &conversation.events {
            if let Some(text) = event.message_text() {
                let role = event.role().unwrap_or("unknown");
                let role_label = if role == "user" { "User" } else { "Assistant" };
                output.push_str(&format!("[{}]\n", role_label));
                output.push_str(&text);
                output.push_str("\n\n");
                output.push_str(&"-".repeat(80));
                output.push_str("\n\n");
            }
        }

        Ok(output)
    }

    /// Export as OpenAI ChatML / fine-tuning messages format.
    /// Produces a JSONL file where each line is a training example with a
    /// "messages" array of {role, content} objects.
    fn export_to_chatml(&self, conversation: &Conversation) -> Result<String> {
        let mut messages = Vec::new();

        // Add system message with conversation context
        let system_msg = serde_json::json!({
            "role": "system",
            "content": format!(
                "You are a helpful coding assistant. This conversation is about: {}",
                conversation.metadata.title.as_deref()
                    .or(conversation.metadata.summary.as_deref())
                    .unwrap_or("a software engineering task")
            )
        });
        messages.push(system_msg);

        for event in &conversation.events {
            if let Some(text) = event.message_text() {
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                let role = event.role().unwrap_or("assistant");
                messages.push(serde_json::json!({
                    "role": role,
                    "content": text
                }));
            }
        }

        // Each JSONL line is a complete training example
        let example = serde_json::json!({ "messages": messages });
        Ok(serde_json::to_string(&example)? + "\n")
    }

    /// Export as ShareGPT format.
    /// Produces a JSON object with "conversations" array of {from, value} pairs.
    fn export_to_sharegpt(&self, conversation: &Conversation) -> Result<String> {
        let mut turns = Vec::new();

        for event in &conversation.events {
            if let Some(text) = event.message_text() {
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                let from = match event.role() {
                    Some("user") => "human",
                    Some("assistant") => "gpt",
                    _ => continue,
                };
                turns.push(serde_json::json!({
                    "from": from,
                    "value": text
                }));
            }
        }

        let output = serde_json::json!({
            "id": conversation.metadata.id,
            "conversations": turns
        });
        Ok(serde_json::to_string_pretty(&output)?)
    }

    /// Export as Alpaca instruction-tuning format.
    /// Extracts user/assistant turn pairs as instruction/output with tool
    /// context as optional input.
    fn export_to_alpaca(&self, conversation: &Conversation) -> Result<String> {
        let mut examples = Vec::new();
        let chat_events: Vec<_> = conversation
            .events
            .iter()
            .filter(|e| e.is_chat_message())
            .collect();

        let mut i = 0;
        while i < chat_events.len() {
            let event = chat_events[i];
            if event.role() == Some("user") {
                let instruction = event.message_text().unwrap_or_default();
                let instruction = instruction.trim().to_string();
                if instruction.is_empty() {
                    i += 1;
                    continue;
                }

                // Look for next assistant response
                let output = if i + 1 < chat_events.len()
                    && chat_events[i + 1].role() == Some("assistant")
                {
                    let resp = chat_events[i + 1]
                        .message_text()
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    i += 2;
                    resp
                } else {
                    i += 1;
                    continue;
                };

                if output.is_empty() {
                    continue;
                }

                examples.push(serde_json::json!({
                    "instruction": instruction,
                    "input": "",
                    "output": output
                }));
            } else {
                i += 1;
            }
        }

        // JSONL: one example per line
        let lines: Vec<String> = examples
            .iter()
            .map(|e| serde_json::to_string(e))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(lines.join("\n") + "\n")
    }
}
