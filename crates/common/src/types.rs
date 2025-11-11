use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fmt;
use uuid::Uuid;
use chrono::{DateTime, Utc};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    pub fn all() -> Vec<Self> {
        vec![
            Self::System,
            Self::Personal,
            Self::Workspace,
            Self::Dependencies,
            Self::Online,
        ]
    }
}

/// Project scope information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectScope {
    pub root: PathBuf,
    pub project_type: ProjectType,
    pub language: Vec<String>,
    pub current_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectType {
    Rust,
    JavaScript,
    Python,
    Mixed,
    Unknown,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

#[derive(Clone, Debug)]
pub struct ProjectScope {
    /// Absolute path to root of project repo or workspace
    pub root_path: String,
    /// Percentage-split of languages present in the project
    pub language_distribution: Vec<(Language, f32)>,
}


// common/types.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentContext {
    /// Normalized project root directory path
    pub project_root: String,

    /// Active or relevant programming/document languages for the task
    pub languages: Vec<String>,

    /// Relevant file types (source, config, docs, etc.)
    pub file_types: Vec<String>,
}
