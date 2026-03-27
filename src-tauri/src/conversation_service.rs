use crate::content_sanitizer::{
    self, chunk_turns, estimate_tokens, extract_text_only, generate_tool_schemas,
    merge_consecutive_turns, sanitize_for_training, should_skip_message, Turn, DEFAULT_MAX_TOKENS,
};
use crate::conversation_analyzer::ConversationAnalyzer;
use crate::error::{ClueditError, Result};
use crate::models::*;
use crate::title_cache::TitleCache;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
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

/// Service for managing conversation files from multiple providers.
/// Designed to be long-lived as Tauri managed state so caches persist.
pub struct ConversationService {
    claude_dir: PathBuf,
    codex_dir: Option<PathBuf>,
    provider: Provider,
    analyzer: ConversationAnalyzer,
    cache: Mutex<HashMap<PathBuf, CachedMetadata>>,
    title_cache: Mutex<TitleCache>,
    codex_history: Mutex<HashMap<String, String>>,
}

impl ConversationService {
    /// Canonicalize `path` and verify it resides within an allowed directory.
    fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        let canonical = dunce::canonicalize(path).map_err(|_| {
            ClueditError::InvalidPath(format!("Path not found: {}", path.display()))
        })?;

        // Check against all allowed roots
        let mut allowed_roots: Vec<&Path> = vec![&self.claude_dir];
        if let Some(ref codex_dir) = self.codex_dir {
            allowed_roots.push(codex_dir);
        }

        // For Codex, also allow the project's cwd path (which is the workspace dir)
        if self.provider == Provider::Codex {
            // Codex project paths are workspace cwds, not inside codex_dir
            // Allow any readable path for Codex conversation listing
            return Ok(canonical);
        }

        for root in &allowed_roots {
            if let Ok(root_canonical) = dunce::canonicalize(root) {
                if canonical.starts_with(&root_canonical) {
                    return Ok(canonical);
                }
            }
        }

        Err(ClueditError::InvalidPath(format!(
            "Path {} is outside allowed directories",
            path.display()
        )))
    }

    pub fn new(data_dir: &Path) -> Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| ClueditError::InvalidPath("Home directory not found".to_string()))?;
        let claude_dir = home.join(".claude");
        let codex_dir = {
            let d = home.join(".codex");
            if d.exists() {
                Some(d)
            } else {
                None
            }
        };

        // Require at least one provider
        if !claude_dir.exists() && codex_dir.is_none() {
            return Err(ClueditError::NotFound(format!(
                "No conversation providers found. Expected Claude at {} or Codex at {}",
                claude_dir.display(),
                home.join(".codex").display()
            )));
        }

        // Load Codex history for titles
        let codex_history = if let Some(ref cd) = codex_dir {
            crate::codex::load_codex_history(cd)
        } else {
            HashMap::new()
        };

        let analyzer = ConversationAnalyzer::new();
        let cache = Mutex::new(HashMap::new());
        let title_cache = Mutex::new(TitleCache::new(data_dir)?);

        // Default to Claude if available, else Codex
        let provider = if claude_dir.exists() {
            Provider::Claude
        } else {
            Provider::Codex
        };

        Ok(Self {
            claude_dir,
            codex_dir,
            provider,
            analyzer,
            cache,
            title_cache,
            codex_history: Mutex::new(codex_history),
        })
    }

    /// Set the active provider.
    pub fn set_provider(&mut self, provider: Provider) {
        self.provider = provider;
    }

    /// Get the active provider.
    pub fn provider(&self) -> Provider {
        self.provider
    }

    /// List available providers.
    pub fn available_providers(&self) -> Vec<ProviderInfo> {
        vec![
            ProviderInfo {
                name: "Claude".to_string(),
                provider: Provider::Claude,
                available: self.claude_dir.exists(),
            },
            ProviderInfo {
                name: "Codex".to_string(),
                provider: Provider::Codex,
                available: self.codex_dir.is_some(),
            },
        ]
    }

    /// List all project directories for the active provider.
    pub fn list_projects(&self) -> Result<Vec<ProjectInfo>> {
        match self.provider {
            Provider::Claude => self.list_claude_projects(),
            Provider::Codex => Ok(self.list_codex_projects()),
        }
    }

    fn list_claude_projects(&self) -> Result<Vec<ProjectInfo>> {
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
                    provider: Provider::Claude,
                });
            }
        }

        projects.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(projects)
    }

    fn list_codex_projects(&self) -> Vec<ProjectInfo> {
        match &self.codex_dir {
            Some(dir) => crate::codex::list_codex_projects(dir),
            None => Vec::new(),
        }
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
        // For Codex, project_path is a workspace cwd — find matching sessions
        let conversation_files: Vec<PathBuf> = if self.provider == Provider::Codex {
            match &self.codex_dir {
                Some(codex_dir) => {
                    crate::codex::codex_sessions_for_project(codex_dir, project_path)
                }
                None => Vec::new(),
            }
        } else {
            let raw_path = PathBuf::from(project_path);
            let path = self.validate_path(&raw_path)?;
            WalkDir::new(&path)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
                .map(|e| e.path().to_path_buf())
                .collect()
        };

        let mut conversations = Vec::new();

        for entry_path_buf in &conversation_files {
            let entry_path = entry_path_buf.as_path();

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

        // For Codex, add title from history by reading session_meta
        if self.provider == Provider::Codex {
            let history = self.codex_history.lock().unwrap();
            for conv in &mut conversations {
                // Read session_meta to get the real session id
                let file_path = PathBuf::from(&conv.file_path);
                if let Some(meta) = crate::codex::read_session_meta(&file_path) {
                    if let Some(title) = history.get(&meta.id) {
                        if conv.title.is_none() {
                            conv.title = Some(title.clone());
                        }
                    }
                }
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
            .map(|file| {
                let reader = BufReader::new(file);
                let mut events = 0;
                let mut messages = 0;

                for line in reader.lines() {
                    events += 1;
                    if let Ok(line) = line {
                        // Claude events
                        let is_claude_msg = (line.contains("\"type\":\"user\"")
                            || line.contains("\"type\":\"assistant\""))
                            && !line.contains("\"isMeta\":true");
                        // Codex events
                        let is_codex_msg = line.contains("\"type\":\"user_message\"")
                            || (line.contains("\"type\":\"message\"")
                                && line.contains("\"role\":\"assistant\""));
                        if is_claude_msg || is_codex_msg {
                            messages += 1;
                        }
                    }
                }

                (events, messages)
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

        let analysis = self.analyzer.analyze(file_path, &id).unwrap_or_else(|_| {
            crate::conversation_analyzer::AnalysisResult {
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
            }
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
        let path = if self.provider == Provider::Codex {
            // Codex session files live under ~/.codex/sessions, validate directly
            if !raw_path.exists() {
                return Err(ClueditError::NotFound(format!(
                    "File not found: {}",
                    raw_path.display()
                )));
            }
            raw_path
        } else {
            self.validate_path(&raw_path)?
        };
        let metadata = self.get_conversation_metadata(&path)?;

        let file = fs::File::open(&path)?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();
        let use_codex_parser = self.provider == Provider::Codex;

        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                if use_codex_parser {
                    // Use Codex parser — converts to ConversationEvent
                    if let Some(event) = crate::codex::parse_codex_line(&line) {
                        events.push(event);
                    }
                } else {
                    // Claude parser
                    match serde_json::from_str::<ConversationEvent>(&line) {
                        Ok(event) => events.push(event),
                        Err(_) => {
                            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                                events.push(ConversationEvent::Unknown(value));
                            }
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
        let lines: Vec<String> = reader.lines().map_while(|l| l.ok()).collect();

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
    pub fn export_conversation(&self, file_path: &str, format: ExportFormat) -> Result<String> {
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
            ExportFormat::ChatMLTools => self.export_to_chatml_tools(&conversation),
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
        output.push_str(&format!(
            "- **File**: {}\n",
            conversation.metadata.file_name
        ));
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

                for line in reader.lines().map_while(|l| l.ok()) {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<ConversationEvent>(&line) {
                        if event.uuid() == Some(target_uuid) {
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

    // ========================================================================
    // Training Export Helpers
    // ========================================================================

    /// Collect text-only, sanitized, merged turns for conversational training.
    /// Strips ALL tool_use/tool_result blocks — keeps only human-readable text.
    fn collect_conversational_turns(conversation: &Conversation) -> Vec<Turn> {
        let mut raw_turns: Vec<Turn> = Vec::new();

        for event in &conversation.events {
            if let Some((role, content)) = extract_text_only(event) {
                if should_skip_message(&content) {
                    continue;
                }

                let clean = sanitize_for_training(&content);
                if clean.is_empty() {
                    continue;
                }

                if role == "assistant" && content_sanitizer::is_low_value_assistant(&clean) {
                    continue;
                }

                raw_turns.push(Turn {
                    role,
                    content: clean,
                });
            }
        }

        merge_consecutive_turns(raw_turns)
    }

    /// Build the system message for training exports
    fn build_system_message(conversation: &Conversation) -> String {
        let mut parts = vec!["You are a helpful coding assistant.".to_string()];

        if let Some(title) = &conversation.metadata.title {
            parts.push(format!("This conversation is about: {}", title));
        }

        if !conversation.metadata.topics.is_empty() {
            parts.push(format!(
                "Topics: {}",
                conversation.metadata.topics.join(", ")
            ));
        }

        parts.join(" ")
    }

    // ========================================================================
    // Conversational Training Exports (text-only, chunked)
    // ========================================================================

    /// Export as OpenAI ChatML fine-tuning format (conversational mode).
    /// Text-only exchanges, no tool noise. Long conversations chunked into
    /// multiple JSONL lines each under the token budget.
    fn export_to_chatml(&self, conversation: &Conversation) -> Result<String> {
        let turns = Self::collect_conversational_turns(conversation);
        if !turns.iter().any(|t| t.role == "user") || !turns.iter().any(|t| t.role == "assistant") {
            return Ok(String::new());
        }

        let system_msg = Self::build_system_message(conversation);
        let system_tokens = estimate_tokens(&system_msg) + 10;
        let chunks = chunk_turns(&turns, DEFAULT_MAX_TOKENS, system_tokens);

        let mut output = String::new();
        for chunk in chunks {
            let mut messages = vec![serde_json::json!({
                "role": "system",
                "content": system_msg
            })];

            for turn in &chunk {
                messages.push(serde_json::json!({
                    "role": turn.role,
                    "content": turn.content
                }));
            }

            // Ensure ends with assistant
            while messages
                .last()
                .is_some_and(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
            {
                messages.pop();
            }

            if messages.len() <= 1 {
                continue;
            }

            let example = serde_json::json!({ "messages": messages });
            output.push_str(&serde_json::to_string(&example)?);
            output.push('\n');
        }

        Ok(output)
    }

    /// Export as ShareGPT format (conversational mode, text-only).
    fn export_to_sharegpt(&self, conversation: &Conversation) -> Result<String> {
        let turns = Self::collect_conversational_turns(conversation);
        if !turns.iter().any(|t| t.role == "user") || !turns.iter().any(|t| t.role == "assistant") {
            return Ok(String::new());
        }

        let sharegpt_turns: Vec<serde_json::Value> = turns
            .iter()
            .map(|t| {
                let from = if t.role == "user" { "human" } else { "gpt" };
                serde_json::json!({ "from": from, "value": t.content })
            })
            .collect();

        let output = serde_json::json!({
            "id": conversation.metadata.id,
            "conversations": sharegpt_turns
        });
        Ok(serde_json::to_string_pretty(&output)?)
    }

    /// Export as Alpaca instruction-tuning format (conversational, text-only).
    fn export_to_alpaca(&self, conversation: &Conversation) -> Result<String> {
        let turns = Self::collect_conversational_turns(conversation);
        let mut examples = Vec::new();

        let mut i = 0;
        while i < turns.len() {
            if turns[i].role == "user" {
                if i + 1 < turns.len() && turns[i + 1].role == "assistant" {
                    let instruction = turns[i].content.trim();
                    let out = turns[i + 1].content.trim();
                    if !instruction.is_empty() && !out.is_empty() {
                        examples.push(serde_json::json!({
                            "instruction": instruction,
                            "input": "",
                            "output": out
                        }));
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        if examples.is_empty() {
            return Ok(String::new());
        }

        let lines: Vec<String> = examples
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(lines.join("\n") + "\n")
    }

    // ========================================================================
    // Agentic Training Export (structured tool calling)
    // ========================================================================

    /// Export as OpenAI ChatML with structured tool_calls (agentic mode).
    /// Produces proper `role: "tool"` messages and `tool_calls` arrays
    /// for training models on tool-calling behavior.
    fn export_to_chatml_tools(&self, conversation: &Conversation) -> Result<String> {
        let system_msg = Self::build_system_message(conversation);
        let mut messages: Vec<serde_json::Value> = vec![serde_json::json!({
            "role": "system",
            "content": system_msg
        })];

        let mut tool_names_seen: Vec<String> = Vec::new();

        for event in &conversation.events {
            match event {
                ConversationEvent::User { message, .. } => {
                    // Handle both text and tool_result blocks from user events.
                    // Tool results appear in non-meta user events in Claude Code JSONL.
                    let mut has_text = false;
                    let mut text_parts: Vec<String> = Vec::new();

                    match &message.content {
                        MessageContent::Text(s) => {
                            let t = s.trim();
                            if !t.is_empty() {
                                text_parts.push(t.to_string());
                            }
                        }
                        MessageContent::Blocks(blocks) => {
                            for block in blocks {
                                match block {
                                    ContentBlock::Text { text } => {
                                        let t = text.trim();
                                        if !t.is_empty() {
                                            text_parts.push(t.to_string());
                                        }
                                    }
                                    ContentBlock::ToolResult {
                                        tool_use_id,
                                        content,
                                        ..
                                    } => {
                                        let result_text = match content {
                                            Some(serde_json::Value::String(s)) => s.clone(),
                                            Some(serde_json::Value::Array(arr)) => arr
                                                .iter()
                                                .filter_map(|v| {
                                                    v.get("text")
                                                        .and_then(|t| t.as_str())
                                                        .map(String::from)
                                                })
                                                .collect::<Vec<_>>()
                                                .join("\n"),
                                            _ => String::new(),
                                        };
                                        if !result_text.is_empty() {
                                            messages.push(serde_json::json!({
                                                "role": "tool",
                                                "tool_call_id": tool_use_id,
                                                "content": content_sanitizer::truncate_utf8(
                                                    &result_text, 4000
                                                )
                                            }));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    // Emit text content as user message (after tool results)
                    if !text_parts.is_empty() {
                        has_text = true;
                        let joined = text_parts.join("\n\n");
                        let clean = sanitize_for_training(&joined);
                        if !clean.is_empty() && !should_skip_message(&clean) {
                            messages.push(serde_json::json!({
                                "role": "user",
                                "content": clean
                            }));
                        }
                    }
                    let _ = has_text; // suppress unused warning
                }
                ConversationEvent::Assistant {
                    message, is_meta, ..
                } if !is_meta => {
                    let mut text_parts: Vec<String> = Vec::new();
                    let mut tool_calls: Vec<serde_json::Value> = Vec::new();

                    match &message.content {
                        MessageContent::Text(s) => {
                            let t = s.trim();
                            if !t.is_empty() {
                                text_parts.push(t.to_string());
                            }
                        }
                        MessageContent::Blocks(blocks) => {
                            for block in blocks {
                                match block {
                                    ContentBlock::Text { text } => {
                                        let t = text.trim();
                                        if !t.is_empty() {
                                            text_parts.push(t.to_string());
                                        }
                                    }
                                    ContentBlock::ToolUse { id, name, input } => {
                                        // Validate tool name for OpenAI compatibility
                                        let safe_name: String = name
                                            .chars()
                                            .filter(|c| {
                                                c.is_alphanumeric() || *c == '_' || *c == '-'
                                            })
                                            .take(64)
                                            .collect();
                                        if safe_name.is_empty() {
                                            continue;
                                        }
                                        if !tool_names_seen.contains(&safe_name) {
                                            tool_names_seen.push(safe_name.clone());
                                        }
                                        tool_calls.push(serde_json::json!({
                                            "id": id,
                                            "type": "function",
                                            "function": {
                                                "name": safe_name,
                                                "arguments": serde_json::to_string(input)
                                                    .unwrap_or_default()
                                            }
                                        }));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    if !tool_calls.is_empty() {
                        let mut msg = serde_json::json!({
                            "role": "assistant",
                            "tool_calls": tool_calls
                        });
                        if !text_parts.is_empty() {
                            let clean = sanitize_for_training(&text_parts.join("\n\n"));
                            if !clean.is_empty() {
                                msg.as_object_mut()
                                    .unwrap()
                                    .insert("content".to_string(), clean.into());
                            }
                        }
                        messages.push(msg);
                    } else if !text_parts.is_empty() {
                        let clean = sanitize_for_training(&text_parts.join("\n\n"));
                        if !clean.is_empty() {
                            messages.push(serde_json::json!({
                                "role": "assistant",
                                "content": clean
                            }));
                        }
                    }
                }
                _ => {}
            }
        }

        // Validate: must end with assistant and have meaningful content
        while messages.last().is_some_and(|m| {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("");
            role == "user" || role == "tool"
        }) {
            messages.pop();
        }

        if messages.len() <= 1 {
            return Ok(String::new());
        }

        // Must have at least one assistant message
        if !messages
            .iter()
            .any(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant"))
        {
            return Ok(String::new());
        }

        // Build tools array from seen tool names
        let tools = generate_tool_schemas(&tool_names_seen);

        let mut example = serde_json::json!({ "messages": messages });
        if !tools.is_empty() {
            example
                .as_object_mut()
                .unwrap()
                .insert("tools".to_string(), serde_json::json!(tools));
        }

        Ok(serde_json::to_string(&example)? + "\n")
    }

    /// Validate an export output path is within safe user directories.
    fn validate_export_path(path: &Path) -> Result<()> {
        let home = dirs::home_dir()
            .ok_or_else(|| ClueditError::InvalidPath("Home directory not found".to_string()))?;
        let allowed = [
            home.join("Downloads"),
            home.join("Documents"),
            home.join("Desktop"),
            home.clone(),
        ];

        // Walk up to nearest existing ancestor for canonicalization
        let mut check = path.to_path_buf();
        loop {
            if check.exists() {
                break;
            }
            if !check.pop() {
                return Err(ClueditError::InvalidPath(
                    "Export path has no valid ancestor".to_string(),
                ));
            }
        }
        let canonical = dunce::canonicalize(&check).map_err(|_| {
            ClueditError::InvalidPath(format!("Cannot resolve path: {}", path.display()))
        })?;
        if !allowed
            .iter()
            .any(|root| dunce::canonicalize(root).is_ok_and(|r| canonical.starts_with(&r)))
        {
            return Err(ClueditError::InvalidPath(format!(
                "Export path {} is not within an allowed directory",
                path.display()
            )));
        }
        Ok(())
    }

    /// Sanitize a string for use as a filename component.
    fn sanitize_filename(s: &str) -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .collect()
    }

    /// Write a single conversation export directly to a file (bypasses frontend FS scope).
    pub fn export_conversation_to_file(
        &self,
        file_path: &str,
        format: ExportFormat,
        output_path: &str,
    ) -> Result<()> {
        let out = PathBuf::from(output_path);
        Self::validate_export_path(&out)?;
        let content = self.export_conversation(file_path, format)?;
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out, content)?;
        Ok(())
    }

    /// Export all conversations to an in-memory string (for HuggingFace publish).
    /// Returns (content, conversations_exported).
    pub fn export_all_to_string(
        &self,
        project_paths: Vec<String>,
        format: ExportFormat,
    ) -> Result<(String, usize)> {
        let effective_paths = if project_paths.is_empty() {
            self.list_projects()?
                .into_iter()
                .map(|p| p.path.to_string_lossy().to_string())
                .collect()
        } else {
            project_paths
        };

        let mut conversation_paths: Vec<PathBuf> = Vec::new();
        for project_path in &effective_paths {
            let raw_path = PathBuf::from(project_path);
            let path = match self.validate_path(&raw_path) {
                Ok(p) => p,
                Err(_) => {
                    // For Codex, scan sessions directly
                    if self.provider == Provider::Codex {
                        if let Some(ref codex_dir) = self.codex_dir {
                            let sessions =
                                crate::codex::codex_sessions_for_project(codex_dir, project_path);
                            conversation_paths.extend(sessions);
                        }
                        continue;
                    }
                    continue;
                }
            };
            for entry in WalkDir::new(&path)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    conversation_paths.push(entry.path().to_path_buf());
                }
            }
        }

        let mut output = String::new();
        let mut exported = 0usize;

        for conv_path in &conversation_paths {
            let file_path_str = conv_path.to_string_lossy().to_string();
            match self.read_conversation(&file_path_str) {
                Ok(conversation) => {
                    if conversation.metadata.total_message_count == 0 {
                        continue;
                    }
                    let result = match format {
                        ExportFormat::ChatML => self.export_to_chatml(&conversation),
                        ExportFormat::ChatMLTools => self.export_to_chatml_tools(&conversation),
                        ExportFormat::ShareGPT => self.export_to_sharegpt(&conversation),
                        ExportFormat::Alpaca => self.export_to_alpaca(&conversation),
                        _ => continue,
                    };
                    if let Ok(content) = result {
                        let trimmed = content.trim();
                        if !trimmed.is_empty() {
                            output.push_str(trimmed);
                            output.push('\n');
                            exported += 1;
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        Ok((output, exported))
    }

    /// Export all conversations in the given projects to a single output file or directory.
    ///
    /// Training formats (ChatML, Alpaca, ShareGPT): concatenated into one file at `output_path`.
    /// Standard formats (Json, JsonPretty, Markdown, Text): one file per conversation in the
    /// directory at `output_path`.
    pub fn export_all_conversations(
        &self,
        project_paths: Vec<String>,
        format: ExportFormat,
        output_path: &str,
    ) -> Result<ExportAllResult> {
        // Validate export path
        Self::validate_export_path(Path::new(output_path))?;

        // If project_paths is empty, export ALL projects
        let effective_paths = if project_paths.is_empty() {
            self.list_projects()?
                .into_iter()
                .map(|p| p.path.to_string_lossy().to_string())
                .collect()
        } else {
            project_paths
        };

        // Collect all conversation file paths
        let mut conversation_paths: Vec<PathBuf> = Vec::new();
        for project_path in &effective_paths {
            let raw_path = PathBuf::from(project_path);
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
                    conversation_paths.push(entry_path.to_path_buf());
                }
            }
        }

        let is_training_format = matches!(
            format,
            ExportFormat::ChatML
                | ExportFormat::Alpaca
                | ExportFormat::ShareGPT
                | ExportFormat::ChatMLTools
        );

        let mut exported = 0usize;
        let mut skipped = 0usize;

        if is_training_format {
            // Concatenate all conversations into a single file
            let out = PathBuf::from(output_path);
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut writer = std::io::BufWriter::new(fs::File::create(&out)?);

            if matches!(format, ExportFormat::ShareGPT) {
                // ShareGPT: collect into a JSON array
                let mut all_entries: Vec<serde_json::Value> = Vec::new();

                for conv_path in &conversation_paths {
                    let file_path_str = conv_path.to_string_lossy().to_string();
                    match self.read_conversation(&file_path_str) {
                        Ok(conversation) => {
                            if conversation.metadata.total_message_count == 0 {
                                skipped += 1;
                                continue;
                            }
                            match self.export_to_sharegpt(&conversation) {
                                Ok(json_str) => {
                                    if let Ok(val) =
                                        serde_json::from_str::<serde_json::Value>(&json_str)
                                    {
                                        all_entries.push(val);
                                        exported += 1;
                                    } else {
                                        skipped += 1;
                                    }
                                }
                                Err(_) => skipped += 1,
                            }
                        }
                        Err(_) => skipped += 1,
                    }
                }

                let array = serde_json::to_string_pretty(&all_entries)?;
                writer.write_all(array.as_bytes())?;
            } else {
                // ChatML / Alpaca: JSONL — each conversation contributes line(s)
                for conv_path in &conversation_paths {
                    let file_path_str = conv_path.to_string_lossy().to_string();
                    match self.read_conversation(&file_path_str) {
                        Ok(conversation) => {
                            if conversation.metadata.total_message_count == 0 {
                                skipped += 1;
                                continue;
                            }
                            let result = match format {
                                ExportFormat::ChatML => self.export_to_chatml(&conversation),
                                ExportFormat::ChatMLTools => {
                                    self.export_to_chatml_tools(&conversation)
                                }
                                ExportFormat::Alpaca => self.export_to_alpaca(&conversation),
                                _ => unreachable!(),
                            };
                            match result {
                                Ok(content) => {
                                    let trimmed = content.trim();
                                    if !trimmed.is_empty() {
                                        writer.write_all(trimmed.as_bytes())?;
                                        writer.write_all(b"\n")?;
                                        exported += 1;
                                    } else {
                                        skipped += 1;
                                    }
                                }
                                Err(_) => skipped += 1,
                            }
                        }
                        Err(_) => skipped += 1,
                    }
                }
            }

            writer.flush()?;
        } else {
            // Standard formats: one file per conversation in the output directory
            let out_dir = PathBuf::from(output_path);
            fs::create_dir_all(&out_dir)?;

            let extension = match format {
                ExportFormat::Json | ExportFormat::JsonPretty => "json",
                ExportFormat::Markdown => "md",
                ExportFormat::Text => "txt",
                _ => "txt",
            };

            for conv_path in &conversation_paths {
                let file_path_str = conv_path.to_string_lossy().to_string();
                match self.read_conversation(&file_path_str) {
                    Ok(conversation) => {
                        if conversation.metadata.total_message_count == 0 {
                            skipped += 1;
                            continue;
                        }
                        let result = match format {
                            ExportFormat::Json => serde_json::to_string(&conversation)
                                .map_err(|e| ClueditError::Export(e.to_string())),
                            ExportFormat::JsonPretty => serde_json::to_string_pretty(&conversation)
                                .map_err(|e| ClueditError::Export(e.to_string())),
                            ExportFormat::Markdown => self.export_to_markdown(&conversation),
                            ExportFormat::Text => self.export_to_text(&conversation),
                            _ => unreachable!(),
                        };
                        match result {
                            Ok(content) => {
                                let filename = format!(
                                    "{}.{}",
                                    Self::sanitize_filename(&conversation.metadata.id),
                                    extension
                                );
                                let file_out = out_dir.join(filename);
                                fs::write(&file_out, content)?;
                                exported += 1;
                            }
                            Err(_) => skipped += 1,
                        }
                    }
                    Err(_) => skipped += 1,
                }
            }
        }

        Ok(ExportAllResult {
            conversations_exported: exported,
            conversations_skipped: skipped,
            output_path: output_path.to_string(),
        })
    }
}
