use anyhow::Error;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf};
use std::fmt::{self};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use strum_macros::EnumIter;
use strum_macros::Display;
use schemars::JsonSchema;
use derive_builder::Builder;
use std::path::Path;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AgentNetworkError>;

use crate::error;

/// Unique identifier for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for conversations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConversationId(pub String);

impl ConversationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ConversationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Collection tier for Qdrant (3-layer data filtering)
#[derive(JsonSchema,Display,Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
pub enum CollectionTier {
    System,        // System files (/etc, man pages)
    Personal,      // Personal documents
    Workspace,     // Active projects
    Dependencies,  // External libraries
    Online,        // Web docs
}

/// Project scope information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectScope {
    pub root: String,
    pub current_file: Option<PathBuf>,
    pub language_distribution: Vec<(Language,f32)>
}

impl ProjectScope{
    pub fn new(root: String, current_file: Option<PathBuf>, language_distribution: Vec<(Language,f32)>)->Self{
        Self{
            root,
            current_file,
            language_distribution,
            }
        }
}


/// Task type for routing decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Coding,
    Documentation,
    Planning,
    Testing,
    Research,
}

/// Agent type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    Orchestrator,
    Coding,
    Planning,
    Writing,
}

impl AgentType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Orchestrator => "orchestrator",
            Self::Coding => "coding",
            Self::Planning => "planning",
            Self::Writing => "writing",
        }
    }
}

/// Document retrieved from RAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub content: String,
    pub file_path: PathBuf,
    pub similarity_score: f32,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub collection_tier: CollectionTier,
    pub language: Option<String>,
    pub file_type: String,
    pub last_modified: DateTime<Utc>,
    pub definitions: Vec<Definition>,
}

/// Message in conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: MessageMetadata,
}

impl Message {
    pub fn new_user(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::User,
            content,
            timestamp: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }

    pub fn new_assistant(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            content,
            timestamp: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub topic_tags: Vec<String>,
    pub file_references: Vec<PathBuf>,
    pub code_snippets: Vec<String>,
}

/// Status events for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StatusEvent {
    TaskStarted {
        task_id: String,
        description: String,
    },
    Decomposition {
        subtasks: Vec<String>,
    },
    Delegation {
        agent: String,
        subtask: String,
    },
    Progress {
        agent: String,
        progress: f32,
        message: String,
    },
    SubtaskComplete {
        agent: String,
        subtask: String,
        result: String,
    },
    AwaitingApproval {
        task: String,
        reason: String,
    },
    Complete {
        result: String,
    },
    Error {
        agent: Option<String>,
        error: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter, Display)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    C,
    Cpp,
    Go,
    Haskell,
    Lua,
    YAML,
    Bash,
    HTML,
    JSON,
    Ruby,
    Asciidoc,
    XML,
    Markdown,
    Yarn,
    Unknown,
}

/// Represents a retrieved context document or snippet for RAG
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Builder)]
pub struct ContextFragment {
    /// The main textual content of the fragment
    pub content: String,

    /// Metadata for the given fragment
    pub metadata: MetadataContextFragment,

    /// Similarity or relevance score (e.g., from retriever)
    pub relevance_score: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Builder)]
pub struct MetadataContextFragment{
    /// Source identifier or file path for provenance
    pub location: Location,

    pub structures: Vec<StructureContextFragment>,

    pub annotations: Option<AnnotationsContextFragment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Builder)]
pub struct StructureContextFragment{
    /// Source identifier or file path for provenance
    kind: String,
    name: Option<String>,
    line_start: usize,
    line_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Builder)]
pub struct AnnotationsContextFragment{
    /// Source identifier or file path for provenance
    pub last_updated: Option<DateTime<Utc>>,
    pub tags: Option<Vec<TagContextFragment>>

}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
pub enum TagContextFragment{
    TAG(String),
    KV(String, String)
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
pub enum Location{
    File{
        path: String,
        line_start: Option<usize>,
        line_end: Option<usize>,
        project_root: Option<String>
    },
    URI{ uri: String }
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Builder)]
pub struct Definition {
    name: String,
    kind: String,
    line: usize,
    byte_range: (usize, usize),
}




#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentNetworkConfig {
    pub agents: Vec<AgentConfig>,
    pub hitl: HitlConfig,
    pub retry: RetryConfig,
    pub token_budget: TokenBudgetConfig,
    pub acp: AcpConfig,
    pub tracing: TracingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub id: String,
    pub agent_type: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HitlConfig {
    pub enabled: bool,
    pub default_mode: HitlMode,
    pub sample_rate: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum HitlMode {
    Blocking,
    Async,
    SampleBased,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub backoff_ms: u64,
    pub backoff_multiplier: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenBudgetConfig {
    pub max_tokens_per_agent: usize,
    pub enable_context_pruning: bool,
    pub enable_prompt_caching: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AcpConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TracingConfig {
    pub enabled: bool,
    pub jaeger_endpoint: Option<String>,
}


#[derive(Debug, Error)]
pub enum AgentNetworkError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Workflow error: {0}")]
    Workflow(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("HITL error: {0}")]
    Hitl(String),

    #[error("File lock error: {0}")]
    FileLock(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Task join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("Channel send error")]
    ChannelSend,

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Error recovery strategy for tasks
#[derive(Debug, Clone)]
pub enum ErrorRecoveryStrategy {
    /// Retry same agent with exponential backoff
    Retry {
        max_attempts: usize,
        backoff_ms: u64,
    },
    /// Switch to backup agent
    SwitchAgent {
        backup_agent_id: String,
    },
    /// Skip task and continue
    Skip,
    /// Request human intervention
    EscalateToHuman,
    /// Abort entire workflow
    Abort,
}

/// Quality evaluation strategy
#[derive(Debug, Clone)]
pub enum QualityStrategy {
    Always,
    OnlyForCritical,
    AfterNIterations(usize),
    Never,
}
