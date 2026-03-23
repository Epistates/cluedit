use crate::conversation_analyzer::truncate_utf8;
use crate::error::Result;
use crate::models::ConversationEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, QueryParser};
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use tokio::sync::Mutex;

/// Search result with highlighted context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastSearchResult {
    pub conversation_id: String,
    pub file_path: String,
    pub title: Option<String>,
    pub project: Option<String>,
    pub snippet: String,
    pub score: f32,
    pub total_matches: usize,
}

/// Indexing progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingProgress {
    pub current: usize,
    pub total: usize,
    pub status: String,
}

/// Fast search indexer using Tantivy
pub struct SearchIndexer {
    index: Arc<Index>,
    schema: Schema,
    writer: Arc<Mutex<IndexWriter>>,
    indexed_conversations: Arc<Mutex<HashMap<String, u64>>>,
}

impl Clone for SearchIndexer {
    fn clone(&self) -> Self {
        Self {
            index: Arc::clone(&self.index),
            schema: self.schema.clone(),
            writer: Arc::clone(&self.writer),
            indexed_conversations: Arc::clone(&self.indexed_conversations),
        }
    }
}

impl SearchIndexer {
    pub fn clone_handle(&self) -> Self {
        self.clone()
    }

    /// Create a new search indexer
    pub fn new(data_dir: &Path) -> Result<Self> {
        let index_path = data_dir.join("search_index");
        fs::create_dir_all(&index_path)?;

        let mut schema_builder = Schema::builder();

        schema_builder.add_text_field("conversation_id", STRING | STORED);
        schema_builder.add_text_field("file_path", STRING | STORED);
        schema_builder.add_text_field("project", STRING | STORED);
        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("content", TEXT);
        schema_builder.add_text_field("user_messages", TEXT);
        schema_builder.add_text_field("assistant_messages", TEXT);
        schema_builder.add_u64_field("modified", INDEXED | STORED);

        let schema = schema_builder.build();

        let index = if index_path.join("meta.json").exists() {
            log::info!("Opening existing search index at {}", index_path.display());
            match Index::open_in_dir(&index_path) {
                Ok(existing_index) => {
                    if existing_index.schema() != schema {
                        log::warn!("Schema mismatch detected — rebuilding search index");
                        drop(existing_index);
                        fs::remove_dir_all(&index_path)?;
                        fs::create_dir_all(&index_path)?;
                        Index::create_in_dir(&index_path, schema.clone())?
                    } else {
                        existing_index
                    }
                }
                Err(e) => {
                    log::warn!("Failed to open search index ({}), recreating...", e);
                    fs::remove_dir_all(&index_path)?;
                    fs::create_dir_all(&index_path)?;
                    Index::create_in_dir(&index_path, schema.clone())?
                }
            }
        } else {
            log::info!("Creating new search index at {}", index_path.display());
            Index::create_in_dir(&index_path, schema.clone())?
        };

        let writer = index.writer(50_000_000)?;

        // Populate indexed_conversations from existing index to avoid re-indexing
        let mut initial_indexed: HashMap<String, u64> = HashMap::new();
        if let Ok(reader) = index.reader() {
            let searcher = reader.searcher();
            let id_field = schema.get_field("conversation_id").unwrap();
            let modified_field = schema.get_field("modified").unwrap();
            if let Ok(doc_addresses) =
                searcher.search(&AllQuery, &tantivy::collector::DocSetCollector)
            {
                for doc_address in doc_addresses {
                    if let Ok(doc) = searcher.doc::<tantivy::TantivyDocument>(doc_address) {
                        let conv_id = doc
                            .get_first(id_field)
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let mtime = doc
                            .get_first(modified_field)
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        if !conv_id.is_empty() {
                            initial_indexed.insert(conv_id, mtime);
                        }
                    }
                }
            }
        }
        log::info!(
            "Loaded {} previously indexed conversations from disk",
            initial_indexed.len()
        );

        Ok(Self {
            index: Arc::new(index),
            schema,
            writer: Arc::new(Mutex::new(writer)),
            indexed_conversations: Arc::new(Mutex::new(initial_indexed)),
        })
    }

    /// Index all conversations in background, emitting progress events
    pub async fn index_all_conversations<F>(
        &self,
        project_paths: Vec<PathBuf>,
        progress_callback: F,
    ) -> Result<()>
    where
        F: Fn(IndexingProgress) + Send + 'static,
    {
        log::info!("Starting background indexing...");

        let mut all_conversations = Vec::new();
        for project_path in &project_paths {
            for entry in walkdir::WalkDir::new(project_path)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    all_conversations.push(path.to_path_buf());
                }
            }
        }

        let total = all_conversations.len();
        log::info!("Found {} conversations to index", total);

        if total == 0 {
            return Ok(());
        }

        progress_callback(IndexingProgress {
            current: 0,
            total,
            status: "Starting indexing...".to_string(),
        });

        let batch_size = 10;
        for (i, chunk) in all_conversations.chunks(batch_size).enumerate() {
            let start_idx = i * batch_size;

            for (j, conv_path) in chunk.iter().enumerate() {
                let current = start_idx + j + 1;

                if let Ok(metadata) = fs::metadata(conv_path) {
                    if let Ok(mtime) = metadata.modified() {
                        let mtime_secs = mtime
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        let id = conv_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();

                        let already_indexed = {
                            let indexed = self.indexed_conversations.lock().await;
                            indexed.get(&id).copied() == Some(mtime_secs)
                        };

                        if already_indexed {
                            progress_callback(IndexingProgress {
                                current,
                                total,
                                status: format!("Indexed {} of {} (cached)", current, total),
                            });
                            continue;
                        }
                    }
                }

                match self.index_conversation(conv_path).await {
                    Ok(_) => {
                        log::debug!("Indexed {} ({}/{})", conv_path.display(), current, total);
                    }
                    Err(e) => {
                        log::warn!("Failed to index {}: {}", conv_path.display(), e);
                    }
                }

                progress_callback(IndexingProgress {
                    current,
                    total,
                    status: format!("Indexing conversation {} of {}", current, total),
                });
            }

            {
                let mut writer = self.writer.lock().await;
                writer.commit()?;
            }
        }

        progress_callback(IndexingProgress {
            current: total,
            total,
            status: "Indexing complete!".to_string(),
        });

        log::info!("Indexing complete! {} conversations indexed", total);
        Ok(())
    }

    /// Index a single conversation file
    async fn index_conversation(&self, file_path: &Path) -> Result<()> {
        let metadata = fs::metadata(file_path)?;
        let id = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mtime = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let project = file_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let (title, user_messages, assistant_messages) = self.parse_conversation(file_path)?;

        let conversation_id_field = self.schema.get_field("conversation_id").unwrap();
        let file_path_field = self.schema.get_field("file_path").unwrap();
        let project_field = self.schema.get_field("project").unwrap();
        let title_field = self.schema.get_field("title").unwrap();
        let content_field = self.schema.get_field("content").unwrap();
        let user_messages_field = self.schema.get_field("user_messages").unwrap();
        let assistant_messages_field = self.schema.get_field("assistant_messages").unwrap();
        let modified_field = self.schema.get_field("modified").unwrap();

        let mut full_content = String::new();
        if let Some(ref t) = title {
            full_content.push_str(t);
            full_content.push(' ');
        }
        full_content.push_str(&user_messages);
        full_content.push(' ');
        full_content.push_str(&assistant_messages);

        let doc = doc!(
            conversation_id_field => id.clone(),
            file_path_field => file_path.to_string_lossy().to_string(),
            project_field => project,
            title_field => title.unwrap_or_default(),
            content_field => full_content,
            user_messages_field => user_messages,
            assistant_messages_field => assistant_messages,
            modified_field => mtime,
        );

        {
            let writer = self.writer.lock().await;
            // Delete any prior document for the same conversation to prevent duplicates
            let id_field = self.schema.get_field("conversation_id").unwrap();
            writer.delete_term(tantivy::Term::from_field_text(id_field, &id));
            writer.add_document(doc)?;
        }

        {
            let mut indexed = self.indexed_conversations.lock().await;
            indexed.insert(id, mtime);
        }

        Ok(())
    }

    /// Parse conversation file to extract searchable text using typed models
    fn parse_conversation(&self, file_path: &Path) -> Result<(Option<String>, String, String)> {
        let file = fs::File::open(file_path)?;
        let reader = std::io::BufReader::new(file);

        let mut title: Option<String> = None;
        let mut user_messages = Vec::new();
        let mut assistant_messages = Vec::new();

        for line in reader.lines().map_while(|l| l.ok()) {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<ConversationEvent>(&line) {
                Ok(event) => match &event {
                    ConversationEvent::User {
                        message, is_meta, ..
                    } => {
                        if !is_meta {
                            let text = message.content.extract_text();
                            if !text.is_empty() {
                                user_messages.push(text);
                            }
                        }
                    }
                    ConversationEvent::Assistant {
                        message, is_meta, ..
                    } => {
                        if !is_meta {
                            let text = message.content.extract_text();
                            if !text.is_empty() {
                                assistant_messages.push(text);
                            }
                        }
                    }
                    ConversationEvent::Summary {
                        summary: Some(s), ..
                    } => {
                        if title.is_none() {
                            title = Some(s.clone());
                        }
                    }
                    _ => {}
                },
                Err(_) => {
                    // Fallback for unknown formats
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                        if title.is_none() {
                            if let Some(t) = obj.get("title").and_then(|v| v.as_str()) {
                                title = Some(t.to_string());
                            }
                        }
                    }
                }
            }
        }

        if title.is_none() && !user_messages.is_empty() {
            title = Some(truncate_utf8(&user_messages[0], 100));
        }

        Ok((title, user_messages.join(" "), assistant_messages.join(" ")))
    }

    /// Search conversations with relevance ranking
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        _fuzzy: bool,
    ) -> Result<Vec<FastSearchResult>> {
        log::debug!("Searching for: '{}'", query);

        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let searcher = reader.searcher();

        let content_field = self.schema.get_field("content").unwrap();
        let title_field = self.schema.get_field("title").unwrap();
        let user_messages_field = self.schema.get_field("user_messages").unwrap();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![title_field, user_messages_field, content_field],
        );

        let parsed_query = query_parser.parse_query(query)?;
        let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(limit))?;

        let conversation_id_field = self.schema.get_field("conversation_id").unwrap();
        let file_path_field = self.schema.get_field("file_path").unwrap();
        let project_field = self.schema.get_field("project").unwrap();

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;

            let conversation_id = retrieved_doc
                .get_first(conversation_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let file_path = retrieved_doc
                .get_first(file_path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let project = retrieved_doc
                .get_first(project_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let title = retrieved_doc
                .get_first(title_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let snippet = if let Some(title_text) = &title {
                truncate_utf8(title_text, 150)
            } else {
                String::new()
            };

            results.push(FastSearchResult {
                conversation_id,
                file_path,
                title,
                project,
                snippet,
                score,
                total_matches: 1,
            });
        }

        Ok(results)
    }

    /// Get index statistics
    pub fn get_stats(&self) -> Result<HashMap<String, usize>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let mut stats = HashMap::new();
        stats.insert(
            "indexed_conversations".to_string(),
            searcher.num_docs() as usize,
        );
        stats.insert("num_segments".to_string(), searcher.segment_readers().len());

        Ok(stats)
    }
}
