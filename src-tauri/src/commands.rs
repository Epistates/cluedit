use crate::error::Result;
use crate::models::*;
use crate::search_indexer::{FastSearchResult, IndexingProgress, SearchIndexer};
use crate::AppState;
use std::path::PathBuf;
use tauri::{Emitter, State};

// ============================================================================
// BACKUP & BRANCH COMMANDS
// ============================================================================

/// Create a full backup of a conversation
#[tauri::command]
pub fn create_backup(
    state: State<'_, AppState>,
    file_path: String,
    label: String,
) -> Result<BackupInfo> {
    state.backup_service.create_backup(&file_path, &label)
}

/// Create a backup truncated at a specific event index (0-based, inclusive)
#[tauri::command]
pub fn create_backup_at_event(
    state: State<'_, AppState>,
    file_path: String,
    event_index: usize,
    label: String,
) -> Result<BackupInfo> {
    state
        .backup_service
        .create_backup_at_event(&file_path, event_index, &label)
}

/// List all backups for a specific conversation
#[tauri::command]
pub fn list_backups(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<BackupInfo>> {
    state.backup_service.list_backups(&conversation_id)
}

/// List all backups across all conversations
#[tauri::command]
pub fn list_all_backups(state: State<'_, AppState>) -> Result<Vec<BackupInfo>> {
    state.backup_service.list_all_backups()
}

/// Restore a conversation from a backup.
/// Returns the safety backup that was automatically created before restoring.
#[tauri::command]
pub fn restore_backup(state: State<'_, AppState>, backup_id: String) -> Result<BackupInfo> {
    state.backup_service.restore_backup(&backup_id)
}

/// Branch a conversation: duplicate with all IDs regenerated.
/// Optionally truncate at a specific event index.
#[tauri::command]
pub fn branch_conversation(
    state: State<'_, AppState>,
    source_path: String,
    truncate_at_event: Option<usize>,
) -> Result<BranchResult> {
    state
        .backup_service
        .branch_conversation(&source_path, truncate_at_event)
}

/// Branch from a backup: create a new conversation from a backup with regenerated IDs.
#[tauri::command]
pub fn branch_from_backup(state: State<'_, AppState>, backup_id: String) -> Result<BranchResult> {
    state.backup_service.branch_from_backup(&backup_id)
}

/// Delete a backup and its file
#[tauri::command]
pub fn delete_backup(state: State<'_, AppState>, backup_id: String) -> Result<()> {
    state.backup_service.delete_backup(&backup_id)
}

/// List all Claude projects
#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectInfo>> {
    state.conversation_service.list_projects()
}

/// List conversations in a specific project
#[tauri::command]
pub fn list_conversations(
    state: State<'_, AppState>,
    project_path: String,
) -> Result<Vec<ConversationMetadata>> {
    state.conversation_service.list_conversations(&project_path)
}

/// Read a full conversation
#[tauri::command]
pub fn read_conversation(state: State<'_, AppState>, file_path: String) -> Result<Conversation> {
    state.conversation_service.read_conversation(&file_path)
}

/// Search across conversations
#[tauri::command]
pub fn search_conversations(
    state: State<'_, AppState>,
    query: String,
    project_paths: Vec<String>,
    case_sensitive: bool,
    use_regex: Option<bool>,
) -> Result<Vec<SearchResult>> {
    state.conversation_service.search_conversations(
        &query,
        project_paths,
        case_sensitive,
        use_regex.unwrap_or(false),
    )
}

/// Export conversation to a specific format
#[tauri::command]
pub fn export_conversation(
    state: State<'_, AppState>,
    file_path: String,
    format: ExportFormat,
) -> Result<String> {
    state
        .conversation_service
        .export_conversation(&file_path, format)
}

/// Export a single conversation directly to a file (bypasses frontend FS scope).
#[tauri::command]
pub fn export_conversation_to_file(
    state: State<'_, AppState>,
    file_path: String,
    format: ExportFormat,
    output_path: String,
) -> Result<()> {
    state
        .conversation_service
        .export_conversation_to_file(&file_path, format, &output_path)
}

/// Export all conversations in the given projects to a file or directory.
/// Pass empty project_paths to export ALL projects.
#[tauri::command]
pub fn export_all_conversations(
    state: State<'_, AppState>,
    project_paths: Vec<String>,
    format: ExportFormat,
    output_path: String,
) -> Result<ExportAllResult> {
    state
        .conversation_service
        .export_all_conversations(project_paths, format, &output_path)
}

/// Get metadata for a single conversation without loading all events
#[tauri::command]
pub fn get_conversation_metadata(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<ConversationMetadata> {
    let path = std::path::PathBuf::from(file_path);
    state.conversation_service.get_conversation_metadata(&path)
}

/// Find the parent conversation file by UUID
#[tauri::command]
pub fn find_parent_conversation(
    state: State<'_, AppState>,
    parent_uuid: String,
) -> Result<Option<String>> {
    state
        .conversation_service
        .find_conversation_by_uuid(&parent_uuid)
}

// ============================================================================
// FAST SEARCH COMMANDS (Tantivy-based)
// ============================================================================

/// Start background indexing of all conversations.
/// Emits "indexing-progress" events to the frontend.
#[tauri::command]
pub async fn start_indexing(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    project_paths: Vec<String>,
) -> Result<()> {
    log::info!(
        "start_indexing called with {} projects",
        project_paths.len()
    );

    let indexer = {
        let mut indexer_lock = state.search_indexer.lock().unwrap();
        if indexer_lock.is_none() {
            *indexer_lock = Some(SearchIndexer::new(&state.data_dir)?);
        }
        indexer_lock.as_ref().unwrap().clone_handle()
    };

    let paths: Vec<PathBuf> = project_paths.iter().map(PathBuf::from).collect();

    let progress_handle = app_handle.clone();

    tokio::spawn(async move {
        let result = indexer
            .index_all_conversations(paths, move |progress: IndexingProgress| {
                if let Err(e) = progress_handle.emit("indexing-progress", &progress) {
                    log::warn!("Failed to emit progress: {}", e);
                }
            })
            .await;

        if let Err(e) = result {
            log::error!("Indexing failed: {}", e);
            let _ = app_handle.emit("indexing-error", format!("Indexing failed: {}", e));
        } else {
            log::info!("Indexing complete!");
            let _ = app_handle.emit("indexing-complete", ());
        }
    });

    Ok(())
}

/// Fast search using Tantivy full-text index
#[tauri::command]
pub async fn fast_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
    fuzzy: Option<bool>,
) -> Result<Vec<FastSearchResult>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let indexer = {
        let mut indexer_lock = state.search_indexer.lock().unwrap();
        if indexer_lock.is_none() {
            *indexer_lock = Some(SearchIndexer::new(&state.data_dir)?);
        }
        indexer_lock.as_ref().unwrap().clone_handle()
    };

    let results = indexer
        .search(&query, limit.unwrap_or(50), fuzzy.unwrap_or(true))
        .await?;

    Ok(results)
}

/// Get search index statistics
#[tauri::command]
pub async fn get_index_stats(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, usize>> {
    let indexer = {
        let mut indexer_lock = state.search_indexer.lock().unwrap();
        if indexer_lock.is_none() {
            *indexer_lock = Some(SearchIndexer::new(&state.data_dir)?);
        }
        indexer_lock.as_ref().unwrap().clone_handle()
    };

    indexer.get_stats()
}
