use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Vector store error: {0}")]
    VectorStore(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Agent not available: {0}")]
    AgentNotAvailable(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("Checkpoint error: {0}")]
    Checkpoint(String),

    #[error("File lock conflict: {path}")]
    FileLockConflict { path: std::path::PathBuf },

    #[error("History error: {0}")]
    History(String),

    #[error("RAG error: {0}")]
    Rag(String),

    #[error("Indexing error: {0}")]
    Indexing(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Convert anyhow errors to AgentError
impl From<anyhow::Error> for AgentError {
    fn from(err: anyhow::Error) -> Self {
        AgentError::Unknown(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AgentError>;
