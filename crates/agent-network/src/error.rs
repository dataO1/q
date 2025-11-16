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


/// Core error type for all agent-network failures
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AgentNetworkError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Configuration validation failed: {details}")]
    ConfigValidation { details: String },

    #[error("Workflow error: {0}")]
    Workflow(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Agent execution failed - {agent_id}: {reason}")]
    AgentExecutionFailed { agent_id: String, reason: String },

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("HITL error: {0}")]
    Hitl(String),

    #[error("File lock error: {0}")]
    FileLock(String),

    #[error("File lock timeout on path: {path}")]
    FileLockTimeout { path: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML deserialization error: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Task join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("Channel send error")]
    ChannelSend,

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Model error: {model}: {reason}")]
    ModelError { model: String, reason: String },

    #[error("Query execution failed: {0}")]
    QueryExecution(String),

    #[error("Orchestration failed: {0}")]
    Orchestration(String),

    #[error("DAG construction failed: {0}")]
    DagConstruction(String),

    #[error("Invalid state transition: {from} -> {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("Resource not found: {resource_type}:{resource_id}")]
    NotFound { resource_type: String, resource_id: String },

    #[error("Timeout: {operation}")]
    Timeout { operation: String },

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

impl AgentNetworkError {
    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a workflow error
    pub fn workflow(msg: impl Into<String>) -> Self {
        Self::Workflow(msg.into())
    }

    /// Create an agent execution error
    pub fn agent_execution(agent_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::AgentExecutionFailed {
            agent_id: agent_id.into(),
            reason: reason.into(),
        }
    }

    /// Create a query execution error
    pub fn query_execution(msg: impl Into<String>) -> Self {
        Self::QueryExecution(msg.into())
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout { .. }
                | Self::ChannelSend
                | Self::Join(_)
                | Self::FileLockTimeout { .. }
        )
    }

    /// Check if error is critical (non-recoverable)
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::Config(_)
                | Self::ConfigValidation { .. }
                | Self::NotFound { .. }
                | Self::DagConstruction(_)
        )
    }
}

pub type AgentResult<T> = std::result::Result<T, AgentError>;
pub type AgentNetworkResult<T> = std::result::Result<T, AgentNetworkError>;
