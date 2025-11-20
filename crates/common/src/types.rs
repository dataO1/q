use serde::{Deserialize, Serialize};
use std::{path::PathBuf};
use std::fmt::{self};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use strum_macros::EnumIter;
use strum_macros::Display;
use schemars::JsonSchema;
use derive_builder::Builder;
use derive_more::Display as MoreDisplay;

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
#[derive(Debug, Clone, Serialize, Deserialize, MoreDisplay)]
#[display("Project Root: {}, Current File: {:?}, Language Distribution: {:?}", root, current_file, language_distribution)]
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


/// Agent type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Display, Hash)]
pub enum AgentType {
    Orchestrator,
    Coding,
    Planning,
    Writing,
    Evaluator,
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

/// HITL approval modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum HitlMode {
    /// Block workflow until human approves
    Blocking,

    /// Continue but flag for review
    Async,

    /// Sample-based review
    SampleBased,
}


impl std::fmt::Display for HitlMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocking => write!(f, "Blocking"),
            Self::Async => write!(f, "Async"),
            Self::SampleBased => write!(f, "SampleBased"),
        }
    }
}

impl Default for HitlMode {
    fn default() -> Self {
        Self::Async
    }
}

/// Error recovery strategy for task failures
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorRecoveryStrategy {
    /// Retry the same agent with exponential backoff
    #[serde(rename = "retry")]
    Retry { max_attempts: usize, backoff_ms: u64 },

    /// Switch to a backup/alternative agent
    #[serde(rename = "switch_agent")]
    SwitchAgent { backup_agent_id: String },

    /// Skip this task and continue workflow
    #[serde(rename = "skip")]
    Skip,

    /// Request human intervention before continuing
    #[serde(rename = "escalate_to_human")]
    EscalateToHuman,

    /// Abort the entire workflow
    #[serde(rename = "abort")]
    Abort,
}

impl ErrorRecoveryStrategy{
    pub fn is_retryable(&self) -> bool{
        match self{
            Self::Retry{max_attempts,backoff_ms} => true,
            _ => false
        }
    }
}

impl fmt::Display for ErrorRecoveryStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retry {
                max_attempts,
                backoff_ms,
            } => write!(
                f,
                "Retry(max_attempts={}, backoff_ms={})",
                max_attempts, backoff_ms
            ),
            Self::SwitchAgent { backup_agent_id } => {
                write!(f, "SwitchAgent(backup={})", backup_agent_id)
            }
            Self::Skip => write!(f, "Skip"),
            Self::EscalateToHuman => write!(f, "EscalateToHuman"),
            Self::Abort => write!(f, "Abort"),
        }
    }
}

/// Quality evaluation strategy for agent outputs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityStrategy {
    /// Always evaluate every output
    #[serde(rename = "always")]
    Always,

    /// Only evaluate high-risk/critical tasks
    #[serde(rename = "only_for_critical")]
    OnlyForCritical,

    /// Evaluate only after N iterations
    #[serde(rename = "after_n_iterations")]
    AfterNIterations(usize),

    /// Never evaluate
    #[serde(rename = "never")]
    Never,
}

impl fmt::Display for QualityStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Always => write!(f, "Always"),
            Self::OnlyForCritical => write!(f, "OnlyForCritical"),
            Self::AfterNIterations(n) => write!(f, "AfterNIterations({})", n),
            Self::Never => write!(f, "Never"),
        }
    }
}
