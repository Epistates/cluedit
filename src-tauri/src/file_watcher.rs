use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    pub fn new(app_handle: AppHandle, watch_paths: Vec<PathBuf>) -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

        for path in &watch_paths {
            if path.exists() {
                watcher.watch(path, RecursiveMode::Recursive)?;
                log::info!("Watching: {}", path.display());
            }
        }

        // Debounced event handler thread
        std::thread::spawn(move || {
            let mut last_emit = Instant::now() - Duration::from_secs(10);
            let debounce = Duration::from_millis(500);

            loop {
                match rx.recv() {
                    Ok(Ok(event)) => {
                        let dominated_by_jsonl = event
                            .paths
                            .iter()
                            .any(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"));

                        let is_relevant =
                            matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
                                && dominated_by_jsonl;

                        if is_relevant && last_emit.elapsed() >= debounce {
                            last_emit = Instant::now();
                            let changed_paths: Vec<String> = event
                                .paths
                                .iter()
                                .map(|p| p.to_string_lossy().to_string())
                                .collect();
                            if let Err(e) = app_handle.emit("conversation-changed", &changed_paths)
                            {
                                log::warn!("Failed to emit conversation-changed: {}", e);
                            }
                        }
                    }
                    Ok(Err(e)) => log::warn!("File watch error: {}", e),
                    Err(_) => break, // Channel closed
                }
            }
        });

        Ok(Self { _watcher: watcher })
    }
}
