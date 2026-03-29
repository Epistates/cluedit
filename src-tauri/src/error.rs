use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClueditError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Export error: {0}")]
    Export(String),

    #[error("HuggingFace auth error: {0}")]
    HfAuth(String),

    #[error("HuggingFace API error: {0}")]
    HfApi(String),

    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Search index error: {0}")]
    TantivyError(#[from] tantivy::TantivyError),

    #[error("Query parser error: {0}")]
    QueryParserError(#[from] tantivy::query::QueryParserError),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, ClueditError>;

/// Extension trait for `Mutex::lock()` that converts poisoned-lock panics into
/// recoverable `ClueditError::Internal` errors.
pub trait MutexExt<T> {
    fn lock_or_err(&self) -> Result<std::sync::MutexGuard<'_, T>>;
}

impl<T> MutexExt<T> for std::sync::Mutex<T> {
    fn lock_or_err(&self) -> Result<std::sync::MutexGuard<'_, T>> {
        self.lock()
            .map_err(|_| ClueditError::Internal("Lock poisoned".to_string()))
    }
}

impl serde::Serialize for ClueditError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
