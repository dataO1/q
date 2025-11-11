use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf};
use std::fmt;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use strum_macros::EnumIter;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
pub enum CollectionTier {
    System,        // System files (/etc, man pages)
    Personal,      // Personal documents
    Workspace,     // Active projects
    Dependencies,  // External libraries
    Online,        // Web docs
}

impl CollectionTier {
    pub fn collection_name(&self) -> &'static str {
        match self {
            Self::System => "system_knowledge",
            Self::Personal => "personal_docs",
            Self::Workspace => "workspace_dev",
            Self::Dependencies => "external_deps",
            Self::Online => "online_docs",
        }
    }
}

/// Project scope information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectScope {
    pub root: PathBuf,
    pub current_file: Option<PathBuf>,
    pub language_distribution: Vec<(Language,f32)>
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
    pub definitions: Vec<String>,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextFragment {
    /// The main textual content of the fragment
    pub content: String,

    /// A short summary or title of the fragment
    pub summary: String,

    /// Source identifier or file path for provenance
    pub source: String,

    /// Similarity or relevance score (e.g., from retriever)
    pub score: f32,
}

impl ContextFragment {
    pub fn new(content: impl Into<String>, summary: impl Into<String>, source: impl Into<String>, score: f32) -> Self {
        Self {
            content: content.into(),
            summary: summary.into(),
            source: source.into(),
            score,
        }
    }
}
