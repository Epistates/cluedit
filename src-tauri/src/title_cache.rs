use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Cached title with file modification time for invalidation
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedTitle {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub total_message_count: usize,
    pub mtime: u64,
}

/// Persistent disk cache for conversation titles
pub struct TitleCache {
    cache_path: PathBuf,
    cache: HashMap<String, CachedTitle>,
}

impl TitleCache {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let cache_path = data_dir.join("cluedit_title_cache.json");

        let cache = if cache_path.exists() {
            let content = fs::read_to_string(&cache_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        Ok(Self { cache_path, cache })
    }

    /// Get cached title if valid (file not modified)
    pub fn get(&self, conversation_id: &str, current_mtime: SystemTime) -> Option<CachedTitle> {
        let cached = self.cache.get(conversation_id)?;
        let current_mtime_secs = current_mtime
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();

        if cached.mtime == current_mtime_secs {
            Some(cached.clone())
        } else {
            None
        }
    }

    /// Store title in cache
    pub fn set(
        &mut self,
        conversation_id: String,
        title: Option<String>,
        summary: Option<String>,
        total_message_count: usize,
        mtime: SystemTime,
    ) {
        let mtime_secs = mtime
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.cache.insert(
            conversation_id,
            CachedTitle {
                title,
                summary,
                total_message_count,
                mtime: mtime_secs,
            },
        );
    }

    /// Persist cache to disk
    pub fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.cache)?;

        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.cache_path, content)?;
        Ok(())
    }
}
