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

    #[error("Search index error: {0}")]
    TantivyError(#[from] tantivy::TantivyError),

    #[error("Query parser error: {0}")]
    QueryParserError(#[from] tantivy::query::QueryParserError),
}

pub type Result<T> = std::result::Result<T, ClueditError>;

impl serde::Serialize for ClueditError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
