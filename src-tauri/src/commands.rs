use crate::error::{MutexExt, Result};
use crate::models::*;
use crate::search_indexer::{FastSearchResult, IndexingProgress, SearchIndexer};
use crate::AppState;
use std::path::PathBuf;
use tauri::{Emitter, State};

// ============================================================================
// PROVIDER COMMANDS
// ============================================================================

/// List available providers (Claude, Codex, etc.)
#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>> {
    let svc = state.conversation_service.lock_or_err()?;
    Ok(svc.available_providers())
}

/// Switch the active provider
#[tauri::command]
pub fn set_provider(state: State<'_, AppState>, provider: Provider) -> Result<()> {
    let mut svc = state.conversation_service.lock_or_err()?;
    svc.set_provider(provider);
    Ok(())
}

// ============================================================================
// HUGGING FACE PUBLISH COMMANDS
// ============================================================================

#[tauri::command]
pub async fn validate_hf_token(token: String) -> Result<crate::hf_publish::WhoamiResponse> {
    crate::hf_publish::validate_token(&token).await
}

#[tauri::command]
pub fn get_hf_token(state: State<'_, AppState>) -> Result<Option<String>> {
    Ok(crate::hf_publish::read_saved_token(&state.data_dir))
}

#[tauri::command]
pub fn get_os_username() -> Option<String> {
    crate::hf_publish::os_username()
}

#[tauri::command]
pub fn save_hf_token(state: State<'_, AppState>, token: String) -> Result<()> {
    crate::hf_publish::save_token(&state.data_dir, token.trim())
}

#[tauri::command]
pub fn delete_hf_token(state: State<'_, AppState>) -> Result<()> {
    crate::hf_publish::delete_token(&state.data_dir);
    Ok(())
}

#[tauri::command]
pub async fn publish_to_huggingface(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    config: crate::hf_publish::PublishConfig,
) -> Result<crate::hf_publish::PublishResult> {
    // Resolve token on backend (never from frontend IPC)
    let token = crate::hf_publish::read_saved_token(&state.data_dir).ok_or_else(|| {
        crate::error::ClueditError::HfAuth(
            "No HuggingFace token found. Please save a token first.".to_string(),
        )
    })?;

    // Export data while holding the lock, then release before HTTP calls
    let (mut content, exported) = {
        let svc = state.conversation_service.lock_or_err()?;
        svc.export_all_to_string(config.project_paths.clone(), config.format.clone())?
    };

    // Apply redaction with per-export CSPRNG key
    if let Some(rc) = config.redact_config.as_ref() {
        // Generate HMAC key from OS CSPRNG for this export session
        let rc = rc.clone().with_hmac_key();
        content = crate::content_sanitizer::redact_sensitive(&content, &rc);
    }

    if exported == 0 {
        return Err(crate::error::ClueditError::Export(
            "No conversations to publish".to_string(),
        ));
    }

    crate::hf_publish::publish_dataset(&config, &token, &content, exported, &app_handle).await
}

// ============================================================================
// BACKUP & BRANCH COMMANDS
// ============================================================================

#[tauri::command]
pub fn create_backup(
    state: State<'_, AppState>,
    file_path: String,
    label: String,
) -> Result<BackupInfo> {
    state.backup_service.create_backup(&file_path, &label)
}

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

#[tauri::command]
pub fn list_backups(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<BackupInfo>> {
    state.backup_service.list_backups(&conversation_id)
}

#[tauri::command]
pub fn list_all_backups(state: State<'_, AppState>) -> Result<Vec<BackupInfo>> {
    state.backup_service.list_all_backups()
}

#[tauri::command]
pub fn restore_backup(state: State<'_, AppState>, backup_id: String) -> Result<BackupInfo> {
    state.backup_service.restore_backup(&backup_id)
}

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

#[tauri::command]
pub fn branch_from_backup(state: State<'_, AppState>, backup_id: String) -> Result<BranchResult> {
    state.backup_service.branch_from_backup(&backup_id)
}

#[tauri::command]
pub fn delete_backup(state: State<'_, AppState>, backup_id: String) -> Result<()> {
    state.backup_service.delete_backup(&backup_id)
}

// ============================================================================
// CONVERSATION COMMANDS
// ============================================================================

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectInfo>> {
    state.conversation_service.lock_or_err()?.list_projects()
}

#[tauri::command]
pub fn list_conversations(
    state: State<'_, AppState>,
    project_path: String,
) -> Result<Vec<ConversationMetadata>> {
    state
        .conversation_service
        .lock()
        .unwrap()
        .list_conversations(&project_path)
}

#[tauri::command]
pub fn read_conversation(state: State<'_, AppState>, file_path: String) -> Result<Conversation> {
    state
        .conversation_service
        .lock()
        .unwrap()
        .read_conversation(&file_path)
}

#[tauri::command]
pub fn search_conversations(
    state: State<'_, AppState>,
    query: String,
    project_paths: Vec<String>,
    case_sensitive: bool,
    use_regex: Option<bool>,
) -> Result<Vec<SearchResult>> {
    state
        .conversation_service
        .lock()
        .unwrap()
        .search_conversations(
            &query,
            project_paths,
            case_sensitive,
            use_regex.unwrap_or(false),
        )
}

#[tauri::command]
pub fn export_conversation(
    state: State<'_, AppState>,
    file_path: String,
    format: ExportFormat,
) -> Result<String> {
    state
        .conversation_service
        .lock()
        .unwrap()
        .export_conversation(&file_path, format)
}

#[tauri::command]
pub fn export_conversation_to_file(
    state: State<'_, AppState>,
    file_path: String,
    format: ExportFormat,
    output_path: String,
) -> Result<()> {
    state
        .conversation_service
        .lock()
        .unwrap()
        .export_conversation_to_file(&file_path, format, &output_path)
}

#[tauri::command]
pub fn export_all_conversations(
    state: State<'_, AppState>,
    project_paths: Vec<String>,
    format: ExportFormat,
    output_path: String,
) -> Result<ExportAllResult> {
    state
        .conversation_service
        .lock()
        .unwrap()
        .export_all_conversations(project_paths, format, &output_path)
}

#[tauri::command]
pub fn get_conversation_metadata(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<ConversationMetadata> {
    let path = std::path::PathBuf::from(file_path);
    state
        .conversation_service
        .lock()
        .unwrap()
        .get_conversation_metadata(&path)
}

#[tauri::command]
pub fn find_parent_conversation(
    state: State<'_, AppState>,
    parent_uuid: String,
) -> Result<Option<String>> {
    state
        .conversation_service
        .lock()
        .unwrap()
        .find_conversation_by_uuid(&parent_uuid)
}

// ============================================================================
// FAST SEARCH COMMANDS (Tantivy-based)
// ============================================================================

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
        let mut indexer_lock = state.search_indexer.lock_or_err()?;
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
        let mut indexer_lock = state.search_indexer.lock_or_err()?;
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

#[tauri::command]
pub async fn get_index_stats(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, usize>> {
    let indexer = {
        let mut indexer_lock = state.search_indexer.lock_or_err()?;
        if indexer_lock.is_none() {
            *indexer_lock = Some(SearchIndexer::new(&state.data_dir)?);
        }
        indexer_lock.as_ref().unwrap().clone_handle()
    };

    indexer.get_stats()
}
