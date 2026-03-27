mod backup_service;
mod codex;
mod commands;
mod content_sanitizer;
mod conversation_analyzer;
mod conversation_service;
mod error;
mod file_watcher;
mod models;
mod search_indexer;
mod title_cache;

use backup_service::BackupService;
use conversation_service::ConversationService;
use search_indexer::SearchIndexer;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Global application state shared across all Tauri commands.
pub struct AppState {
    pub conversation_service: Mutex<ConversationService>,
    pub backup_service: BackupService,
    pub search_indexer: Arc<Mutex<Option<SearchIndexer>>>,
    pub data_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data directory");

            std::fs::create_dir_all(&data_dir).expect("Failed to create app data directory");

            let home = dirs::home_dir().expect("Home directory not found");

            let conversation_service = ConversationService::new(&data_dir).unwrap_or_else(|e| {
                panic!(
                    "Failed to initialize ConversationService: {}. Expected data at {} or {}",
                    e,
                    home.join(".claude").display(),
                    home.join(".codex").display()
                )
            });

            let backup_service =
                BackupService::new(&data_dir).expect("Failed to initialize BackupService");

            app.manage(AppState {
                conversation_service: Mutex::new(conversation_service),
                backup_service,
                search_indexer: Arc::new(Mutex::new(None)),
                data_dir,
            });

            // Start file watcher for live refresh — watch both Claude and Codex dirs
            let mut watch_paths = Vec::new();
            let claude_projects_dir = home.join(".claude").join("projects");
            if claude_projects_dir.exists() {
                watch_paths.push(claude_projects_dir);
            }
            let codex_sessions_dir = home.join(".codex").join("sessions");
            if codex_sessions_dir.exists() {
                watch_paths.push(codex_sessions_dir);
            }

            if !watch_paths.is_empty() {
                match file_watcher::FileWatcher::new(app.handle().clone(), watch_paths) {
                    Ok(watcher) => {
                        app.manage(watcher);
                        log::info!("File watcher started");
                    }
                    Err(e) => {
                        log::warn!("Failed to start file watcher: {}", e);
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_projects,
            commands::list_conversations,
            commands::read_conversation,
            commands::search_conversations,
            commands::export_conversation,
            commands::export_conversation_to_file,
            commands::export_all_conversations,
            commands::get_conversation_metadata,
            commands::find_parent_conversation,
            commands::start_indexing,
            commands::fast_search,
            commands::get_index_stats,
            // Provider commands
            commands::list_providers,
            commands::set_provider,
            // Backup & branch
            commands::create_backup,
            commands::create_backup_at_event,
            commands::list_backups,
            commands::list_all_backups,
            commands::restore_backup,
            commands::branch_conversation,
            commands::branch_from_backup,
            commands::delete_backup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
