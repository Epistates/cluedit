mod commands;
mod conversation_analyzer;
mod conversation_service;
mod error;
mod file_watcher;
mod models;
mod search_indexer;
mod title_cache;

use conversation_service::ConversationService;
use search_indexer::SearchIndexer;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Global application state shared across all Tauri commands.
/// Both services are created once at startup and persist for the app lifetime,
/// ensuring caches are actually reused across IPC calls.
pub struct AppState {
    pub conversation_service: ConversationService,
    pub search_indexer: Arc<Mutex<Option<SearchIndexer>>>,
    pub data_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
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

            std::fs::create_dir_all(&data_dir)
                .expect("Failed to create app data directory");

            let conversation_service = ConversationService::new(&data_dir)
                .expect("Failed to initialize ConversationService — is ~/.claude/ present?");

            app.manage(AppState {
                conversation_service,
                search_indexer: Arc::new(Mutex::new(None)),
                data_dir,
            });

            // Start file watcher for live refresh
            let claude_projects_dir = {
                let home = dirs::home_dir().expect("Home directory not found");
                home.join(".claude").join("projects")
            };
            if claude_projects_dir.exists() {
                match file_watcher::FileWatcher::new(
                    app.handle().clone(),
                    vec![claude_projects_dir],
                ) {
                    Ok(watcher) => {
                        // Keep watcher alive by storing it in managed state
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
            commands::get_conversation_metadata,
            commands::find_parent_conversation,
            commands::start_indexing,
            commands::fast_search,
            commands::get_index_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
