use crate::error::{ClueditError, Result};
use crate::models::{BackupInfo, BranchResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

/// Maximum length for user-supplied backup labels
const MAX_LABEL_LENGTH: usize = 256;

// ============================================================================
// Backup Manifest (persistent tracking of all backups)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupManifest {
    version: u32,
    backups: Vec<BackupInfo>,
}

impl BackupManifest {
    fn new() -> Self {
        Self {
            version: 1,
            backups: Vec::new(),
        }
    }

    fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::new())
        }
    }

    /// Atomic save: write to temp file, then rename over the target.
    /// Prevents manifest corruption if the process crashes mid-write.
    fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &content)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

// ============================================================================
// ID Remapper — consistent UUID/ID regeneration for branching
// ============================================================================

struct IdRemapper {
    map: HashMap<String, String>,
}

impl IdRemapper {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Get or create a new UUID for the given old ID.
    /// The same old ID always maps to the same new ID within a single remap session.
    fn remap(&mut self, old_id: &str) -> String {
        self.map
            .entry(old_id.to_string())
            .or_insert_with(|| Uuid::new_v4().to_string())
            .clone()
    }

    fn count(&self) -> usize {
        self.map.len()
    }
}

// ============================================================================
// Backup Service
// ============================================================================

pub struct BackupService {
    backups_dir: PathBuf,
    manifest_path: PathBuf,
    claude_dir: PathBuf,
    /// Serializes all manifest read-modify-write cycles
    manifest_lock: Mutex<()>,
}

impl BackupService {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let backups_dir = data_dir.join("backups");
        fs::create_dir_all(&backups_dir)?;

        let manifest_path = backups_dir.join("manifest.json");
        let home = dirs::home_dir()
            .ok_or_else(|| ClueditError::InvalidPath("Home directory not found".to_string()))?;
        let claude_dir = home.join(".claude");

        Ok(Self {
            backups_dir,
            manifest_path,
            claude_dir,
            manifest_lock: Mutex::new(()),
        })
    }

    /// Validate that a path is within the Claude directory
    fn validate_claude_path(&self, path: &Path) -> Result<PathBuf> {
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

    /// Validate that a path is within the backups directory
    fn validate_backup_path(&self, path: &Path) -> Result<PathBuf> {
        let canonical = path.canonicalize().map_err(|_| {
            ClueditError::InvalidPath(format!("Backup file not found: {}", path.display()))
        })?;
        let backups_canonical = self.backups_dir.canonicalize().map_err(|_| {
            ClueditError::InvalidPath("Backups directory not accessible".to_string())
        })?;
        if !canonical.starts_with(&backups_canonical) {
            return Err(ClueditError::InvalidPath(format!(
                "Backup file {} is outside the backups directory",
                path.display()
            )));
        }
        Ok(canonical)
    }

    /// Read non-empty lines from a file
    fn read_lines(path: &Path) -> Result<Vec<String>> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        Ok(reader
            .lines()
            .map_while(|l| l.ok())
            .filter(|l| !l.trim().is_empty())
            .collect())
    }

    // ========================================================================
    // Backup Operations
    // ========================================================================

    /// Create a full backup of a conversation
    pub fn create_backup(&self, source_path: &str, label: &str) -> Result<BackupInfo> {
        self.create_backup_internal(source_path, label, None, false)
    }

    /// Create a backup truncated at a specific event index (0-based, inclusive)
    pub fn create_backup_at_event(
        &self,
        source_path: &str,
        event_index: usize,
        label: &str,
    ) -> Result<BackupInfo> {
        self.create_backup_internal(source_path, label, Some(event_index), false)
    }

    fn create_backup_internal(
        &self,
        source_path: &str,
        label: &str,
        truncate_at: Option<usize>,
        auto_backup: bool,
    ) -> Result<BackupInfo> {
        let raw_path = PathBuf::from(source_path);
        let source = self.validate_claude_path(&raw_path)?;

        let conversation_id = source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Create backup directory for this conversation
        let conv_backup_dir = self.backups_dir.join(&conversation_id);
        fs::create_dir_all(&conv_backup_dir)?;

        let backup_id = Uuid::new_v4().to_string();
        let backup_path = conv_backup_dir.join(format!("{}.jsonl", backup_id));

        // Read non-empty source lines
        let lines = Self::read_lines(&source)?;

        // Write backup (optionally truncated)
        let event_count = if let Some(idx) = truncate_at {
            let count = (idx + 1).min(lines.len());
            let mut writer = fs::File::create(&backup_path)?;
            for line in &lines[..count] {
                writeln!(writer, "{}", line)?;
            }
            count
        } else {
            // For full backups, write filtered lines (no blank lines)
            let mut writer = fs::File::create(&backup_path)?;
            for line in &lines {
                writeln!(writer, "{}", line)?;
            }
            lines.len()
        };

        let size_bytes = fs::metadata(&backup_path)?.len();

        // Truncate label to prevent manifest inflation
        let safe_label = if label.len() > MAX_LABEL_LENGTH {
            &label[..MAX_LABEL_LENGTH]
        } else {
            label
        };

        let info = BackupInfo {
            id: backup_id,
            conversation_id,
            original_file_path: source.to_string_lossy().to_string(),
            backup_file_path: backup_path.to_string_lossy().to_string(),
            label: safe_label.to_string(),
            created_at: Utc::now().to_rfc3339(),
            event_count,
            truncated_at_event: truncate_at,
            size_bytes,
            auto_backup,
        };

        // Serialize manifest access
        let _lock = self.manifest_lock.lock().unwrap();
        let mut manifest = BackupManifest::load(&self.manifest_path)?;
        manifest.backups.push(info.clone());
        manifest.save(&self.manifest_path)?;

        log::info!(
            "Backup created: {} ({} events, {} bytes)",
            info.id,
            info.event_count,
            info.size_bytes
        );

        Ok(info)
    }

    /// List all backups for a specific conversation
    pub fn list_backups(&self, conversation_id: &str) -> Result<Vec<BackupInfo>> {
        let _lock = self.manifest_lock.lock().unwrap();
        let manifest = BackupManifest::load(&self.manifest_path)?;
        Ok(manifest
            .backups
            .into_iter()
            .filter(|b| b.conversation_id == conversation_id)
            .collect())
    }

    /// List all backups across all conversations
    pub fn list_all_backups(&self) -> Result<Vec<BackupInfo>> {
        let _lock = self.manifest_lock.lock().unwrap();
        let manifest = BackupManifest::load(&self.manifest_path)?;
        Ok(manifest.backups)
    }

    /// Restore a conversation from a backup.
    /// Automatically creates a safety backup of the current state first.
    /// Returns the safety backup info so the user can undo the restore.
    pub fn restore_backup(&self, backup_id: &str) -> Result<BackupInfo> {
        // Hold the lock for the entire restore operation (safety backup + copy)
        let _lock = self.manifest_lock.lock().unwrap();

        let manifest = BackupManifest::load(&self.manifest_path)?;
        let backup = manifest
            .backups
            .iter()
            .find(|b| b.id == backup_id)
            .ok_or_else(|| ClueditError::NotFound(format!("Backup not found: {}", backup_id)))?
            .clone();

        // Validate backup file is within our backups directory
        let backup_file = self.validate_backup_path(&PathBuf::from(&backup.backup_file_path))?;

        // Validate original path is within the Claude directory
        let original_path =
            self.validate_claude_path(&PathBuf::from(&backup.original_file_path))?;

        // Ensure the target directory exists (handles case where project dir was recreated)
        if let Some(parent) = original_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Safety backup of current state before overwriting (only if the file still exists)
        let safety_backup = if original_path.exists() {
            let safety = self.create_backup_internal_locked(
                &backup.original_file_path,
                &format!("Auto-backup before restoring '{}'", backup.label),
                None,
                true,
            )?;
            Some(safety)
        } else {
            None
        };

        // Restore: copy backup over original
        fs::copy(&backup_file, &original_path)?;

        log::info!(
            "Restored backup {} to {}{}",
            backup_id,
            backup.original_file_path,
            safety_backup
                .as_ref()
                .map(|s| format!(". Safety backup: {}", s.id))
                .unwrap_or_default()
        );

        // Return safety backup if created, otherwise synthesize info about what happened
        Ok(safety_backup.unwrap_or(BackupInfo {
            id: String::new(),
            conversation_id: backup.conversation_id,
            original_file_path: backup.original_file_path,
            backup_file_path: String::new(),
            label: "No safety backup needed (original was missing)".to_string(),
            created_at: Utc::now().to_rfc3339(),
            event_count: 0,
            truncated_at_event: None,
            size_bytes: 0,
            auto_backup: true,
        }))
    }

    /// Internal backup creation that assumes the manifest lock is already held
    fn create_backup_internal_locked(
        &self,
        source_path: &str,
        label: &str,
        truncate_at: Option<usize>,
        auto_backup: bool,
    ) -> Result<BackupInfo> {
        let raw_path = PathBuf::from(source_path);
        let source = self.validate_claude_path(&raw_path)?;

        let conversation_id = source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let conv_backup_dir = self.backups_dir.join(&conversation_id);
        fs::create_dir_all(&conv_backup_dir)?;

        let backup_id = Uuid::new_v4().to_string();
        let backup_path = conv_backup_dir.join(format!("{}.jsonl", backup_id));

        let lines = Self::read_lines(&source)?;

        let event_count = if let Some(idx) = truncate_at {
            let count = (idx + 1).min(lines.len());
            let mut writer = fs::File::create(&backup_path)?;
            for line in &lines[..count] {
                writeln!(writer, "{}", line)?;
            }
            count
        } else {
            let mut writer = fs::File::create(&backup_path)?;
            for line in &lines {
                writeln!(writer, "{}", line)?;
            }
            lines.len()
        };

        let size_bytes = fs::metadata(&backup_path)?.len();

        let safe_label = if label.len() > MAX_LABEL_LENGTH {
            &label[..MAX_LABEL_LENGTH]
        } else {
            label
        };

        let info = BackupInfo {
            id: backup_id,
            conversation_id,
            original_file_path: source.to_string_lossy().to_string(),
            backup_file_path: backup_path.to_string_lossy().to_string(),
            label: safe_label.to_string(),
            created_at: Utc::now().to_rfc3339(),
            event_count,
            truncated_at_event: truncate_at,
            size_bytes,
            auto_backup,
        };

        // Caller already holds the lock — update manifest directly
        let mut manifest = BackupManifest::load(&self.manifest_path)?;
        manifest.backups.push(info.clone());
        manifest.save(&self.manifest_path)?;

        Ok(info)
    }

    /// Delete a backup and its file
    pub fn delete_backup(&self, backup_id: &str) -> Result<()> {
        let _lock = self.manifest_lock.lock().unwrap();

        let mut manifest = BackupManifest::load(&self.manifest_path)?;
        let idx = manifest
            .backups
            .iter()
            .position(|b| b.id == backup_id)
            .ok_or_else(|| ClueditError::NotFound(format!("Backup not found: {}", backup_id)))?;

        // Delete the file first, then update manifest.
        // If manifest save fails, the entry still points to a missing file (safe).
        // If we did it the other way, a crash would orphan a file with no manifest entry.
        let backup = &manifest.backups[idx];
        let backup_path = PathBuf::from(&backup.backup_file_path);
        // Validate path is within backups directory before deleting
        if let Err(e) = self.validate_backup_path(&backup_path) {
            log::warn!("Skipping deletion of invalid backup path: {}", e);
        } else if backup_path.exists() {
            fs::remove_file(&backup_path)?;
        }

        let conversation_id = backup.conversation_id.clone();
        manifest.backups.remove(idx);
        manifest.save(&self.manifest_path)?;

        // Clean up empty conversation backup directory
        let conv_dir = self.backups_dir.join(&conversation_id);
        if conv_dir.exists() {
            if let Ok(mut entries) = fs::read_dir(&conv_dir) {
                if entries.next().is_none() {
                    let _ = fs::remove_dir(&conv_dir);
                }
            }
        }

        log::info!("Deleted backup: {}", backup_id);
        Ok(())
    }

    // ========================================================================
    // Branch Operations
    // ========================================================================

    /// Branch a conversation: duplicate with all IDs regenerated.
    /// Optionally truncate at a specific event index (0-based, inclusive).
    /// The new file is placed in the same project directory as the original.
    pub fn branch_conversation(
        &self,
        source_path: &str,
        truncate_at_event: Option<usize>,
    ) -> Result<BranchResult> {
        let raw_path = PathBuf::from(source_path);
        let source = self.validate_claude_path(&raw_path)?;

        let target_dir = source
            .parent()
            .ok_or_else(|| {
                ClueditError::InvalidPath("Cannot determine project directory".to_string())
            })?
            .to_path_buf();

        self.branch_from_source(&source, &target_dir, truncate_at_event)
    }

    /// Branch from a backup: create a new conversation from a backup with regenerated IDs.
    /// The new file is placed in the original conversation's project directory.
    pub fn branch_from_backup(&self, backup_id: &str) -> Result<BranchResult> {
        let _lock = self.manifest_lock.lock().unwrap();

        let manifest = BackupManifest::load(&self.manifest_path)?;
        let backup = manifest
            .backups
            .iter()
            .find(|b| b.id == backup_id)
            .ok_or_else(|| ClueditError::NotFound(format!("Backup not found: {}", backup_id)))?
            .clone();

        // Validate backup file is within our backups directory
        let source = self.validate_backup_path(&PathBuf::from(&backup.backup_file_path))?;

        // Validate and resolve original path to determine target directory
        let original_path =
            self.validate_claude_path(&PathBuf::from(&backup.original_file_path))?;
        let target_dir = original_path
            .parent()
            .ok_or_else(|| {
                ClueditError::InvalidPath("Cannot determine original project directory".to_string())
            })?
            .to_path_buf();

        if !target_dir.exists() {
            return Err(ClueditError::NotFound(format!(
                "Original project directory missing: {}",
                target_dir.display()
            )));
        }

        self.branch_from_source(&source, &target_dir, None)
    }

    /// Core branching logic: read source, remap all IDs, write to target directory.
    fn branch_from_source(
        &self,
        source: &Path,
        target_dir: &Path,
        truncate_at: Option<usize>,
    ) -> Result<BranchResult> {
        // Read all lines as raw JSON to preserve every field
        let file = fs::File::open(source)?;
        let reader = BufReader::new(file);
        let mut raw_lines: Vec<serde_json::Value> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(value) => raw_lines.push(value),
                    Err(e) => {
                        log::warn!("Skipping unparseable line during branch: {}", e);
                    }
                }
            }
        }

        // Truncate if requested
        let lines_to_process = if let Some(idx) = truncate_at {
            let end = (idx + 1).min(raw_lines.len());
            &raw_lines[..end]
        } else {
            &raw_lines[..]
        };

        // Remap all IDs
        let mut remapper = IdRemapper::new();
        let mut remapped_lines: Vec<serde_json::Value> = Vec::with_capacity(lines_to_process.len());
        let mut cleared_logical_parent = false;

        for line in lines_to_process.iter() {
            let mut value = line.clone();

            // Clear logicalParentUuid on the first USER event to break continuation link.
            // Must target the actual first user event, not just index 0 (which may be a system event).
            if !cleared_logical_parent {
                let is_user = value.get("type").and_then(|t| t.as_str()) == Some("user");
                if is_user {
                    if let Some(obj) = value.as_object_mut() {
                        obj.remove("logicalParentUuid");
                    }
                    cleared_logical_parent = true;
                }
            }

            Self::remap_event_ids(&mut value, &mut remapper);
            remapped_lines.push(value);
        }

        // Generate new conversation ID and write to target directory
        let new_id = Uuid::new_v4().to_string();
        let new_path = target_dir.join(format!("{}.jsonl", new_id));

        let mut writer = fs::File::create(&new_path)?;
        for value in &remapped_lines {
            let serialized = serde_json::to_string(value)?;
            writeln!(writer, "{}", serialized)?;
        }

        let result = BranchResult {
            new_file_path: new_path.to_string_lossy().to_string(),
            new_conversation_id: new_id.clone(),
            event_count: remapped_lines.len(),
            ids_remapped: remapper.count(),
        };

        log::info!(
            "Branched conversation: {} ({} events, {} IDs remapped)",
            new_id,
            result.event_count,
            result.ids_remapped
        );

        Ok(result)
    }

    // ========================================================================
    // ID Remapping internals
    // ========================================================================

    /// Remap all identifiers within a single JSONL event.
    /// Handles:
    ///   - Top-level event fields: uuid, parentUuid, sessionId, requestId,
    ///     agentId, promptId, logicalParentUuid, sourceToolUseId, etc.
    ///   - Nested message.id
    ///   - Content block IDs: tool_use.id, tool_result.tool_use_id
    ///   - Progress event tool IDs
    ///   - file-history-snapshot messageId
    ///   - toolUseResult.tool_use_id
    fn remap_event_ids(value: &mut serde_json::Value, remapper: &mut IdRemapper) {
        // Top-level event identifiers
        Self::remap_string_field(value, "uuid", remapper);
        Self::remap_string_field(value, "parentUuid", remapper);
        Self::remap_string_field(value, "sessionId", remapper);
        Self::remap_string_field(value, "requestId", remapper);
        Self::remap_string_field(value, "agentId", remapper);
        Self::remap_string_field(value, "promptId", remapper);
        Self::remap_string_field(value, "logicalParentUuid", remapper);
        Self::remap_string_field(value, "sourceToolUseId", remapper);
        Self::remap_string_field(value, "sourceToolAssistantUuid", remapper);
        Self::remap_string_field(value, "toolUseID", remapper);
        Self::remap_string_field(value, "parentToolUseID", remapper);
        Self::remap_string_field(value, "messageId", remapper);

        // Nested message object
        if let Some(message) = value.get_mut("message") {
            Self::remap_string_field(message, "id", remapper);

            // Walk content blocks for tool_use/tool_result IDs
            if let Some(content) = message.get_mut("content") {
                if let Some(blocks) = content.as_array_mut() {
                    for block in blocks {
                        if let Some(block_type) = block
                            .get("type")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string())
                        {
                            match block_type.as_str() {
                                "tool_use" => {
                                    Self::remap_string_field(block, "id", remapper);
                                }
                                "tool_result" => {
                                    Self::remap_string_field(block, "tool_use_id", remapper);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // toolUseResult may contain nested tool_use_id reference
        if let Some(tool_result) = value.get_mut("toolUseResult") {
            Self::remap_string_field(tool_result, "tool_use_id", remapper);
        }
    }

    /// Remap a single string field in a JSON object using the remapper.
    /// No-op if the field doesn't exist, isn't a string, or is empty.
    fn remap_string_field(value: &mut serde_json::Value, field: &str, remapper: &mut IdRemapper) {
        if let Some(obj) = value.as_object_mut() {
            if let Some(serde_json::Value::String(old_id)) = obj.get(field) {
                if !old_id.is_empty() {
                    let new_id = remapper.remap(old_id);
                    obj.insert(field.to_string(), serde_json::Value::String(new_id));
                }
            }
        }
    }
}
