//! Data models for the ACP TUI application
//!
//! This module contains the core data structures used throughout the application,
//! including state models, event types, and view models for the UI components.

use chrono::{DateTime, Utc};
use petgraph::Graph;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Application state
#[derive(Debug, Clone)]
pub struct AppState {
    /// Current conversation being tracked
    pub current_conversation: Option<Conversation>,
    
    /// History of conversations
    pub conversation_history: VecDeque<Conversation>,
    
    /// Server capabilities
    pub capabilities: Option<crate::client::CapabilitiesResponse>,
    
    /// Connection status
    pub connection_status: ConnectionStatus,
    
    /// UI state
    pub ui_state: UiState,
}

/// Connection status
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: usize },
    Failed { error: String },
}

/// UI state
#[derive(Debug, Clone)]
pub struct UiState {
    /// Current input text
    pub input_text: String,
    
    /// Input cursor position
    pub input_cursor: usize,
    
    /// Currently selected UI component
    pub focus: FocusState,
    
    /// Scroll positions
    pub scroll_positions: ScrollPositions,
    
    /// UI dimensions
    pub dimensions: UiDimensions,
}

/// Focus state for UI components
#[derive(Debug, Clone, PartialEq)]
pub enum FocusState {
    QueryInput,
    HistoryView,
    DagView,
}

/// Scroll positions for different views
#[derive(Debug, Clone)]
pub struct ScrollPositions {
    pub history_scroll: usize,
    pub dag_scroll: usize,
}

/// UI dimensions
#[derive(Debug, Clone)]
pub struct UiDimensions {
    pub terminal_width: u16,
    pub terminal_height: u16,
    pub history_width: u16,
    pub dag_width: u16,
}

/// Represents a conversation with the ACP server
#[derive(Debug, Clone)]
pub struct Conversation {
    /// Unique identifier
    pub id: String,
    
    /// Original query
    pub query: String,
    
    /// When the conversation started
    pub started_at: DateTime<Utc>,
    
    /// Current status
    pub status: ConversationStatus,
    
    /// Workflow state (if available)
    pub workflow: Option<WorkflowState>,
    
    /// History of events
    pub events: Vec<StatusEvent>,
    
    /// Final result (if completed)
    pub result: Option<String>,
}

/// Status of a conversation
#[derive(Debug, Clone, PartialEq)]
pub enum ConversationStatus {
    Started,
    Planning,
    Executing,
    Completed,
    Failed { error: String },
}

/// Workflow execution state
#[derive(Debug, Clone)]
pub struct WorkflowState {
    /// DAG representation of the workflow
    pub dag: Graph<WorkflowNode, WorkflowEdge>,
    
    /// Current wave being executed
    pub current_wave: Option<usize>,
    
    /// Total number of waves
    pub total_waves: usize,
    
    /// Mapping from node IDs to graph indices
    pub node_map: HashMap<String, petgraph::graph::NodeIndex>,
    
    /// Wave structure
    pub waves: Vec<ExecutionWave>,
}

/// Node in the workflow DAG
#[derive(Debug, Clone)]
pub struct WorkflowNode {
    /// Unique node identifier
    pub id: String,
    
    /// Node type
    pub node_type: NodeType,
    
    /// Current status
    pub status: NodeStatus,
    
    /// Agent information (if applicable)
    pub agent_info: Option<AgentInfo>,
    
    /// Start and end times
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Type of workflow node
#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Planning,
    Agent { agent_type: String },
    Tool { tool_name: String },
    Evaluation,
    Merge,
}

/// Status of a workflow node
#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed { error: String },
    Skipped,
}

/// Agent information
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub agent_id: String,
    pub agent_type: String,
    pub current_activity: Option<AgentActivity>,
    pub context_size: Option<usize>,
}

/// Current agent activity
#[derive(Debug, Clone)]
pub struct AgentActivity {
    pub activity_type: ActivityType,
    pub description: String,
    pub started_at: DateTime<Utc>,
}

/// Type of agent activity
#[derive(Debug, Clone, PartialEq)]
pub enum ActivityType {
    Thinking,
    RagRetrieval,
    ToolExecution { tool_name: String },
    HitlWaiting,
}

/// Edge in the workflow DAG
#[derive(Debug, Clone)]
pub struct WorkflowEdge {
    pub dependency_type: DependencyType,
}

/// Type of dependency between nodes
#[derive(Debug, Clone, PartialEq)]
pub enum DependencyType {
    Sequential,
    DataDependency,
    ConditionalDependency,
}

/// Execution wave containing parallel tasks
#[derive(Debug, Clone)]
pub struct ExecutionWave {
    pub wave_index: usize,
    pub nodes: Vec<String>, // Node IDs in this wave
    pub status: WaveStatus,
}

/// Status of an execution wave
#[derive(Debug, Clone, PartialEq)]
pub enum WaveStatus {
    Pending,
    Running,
    Completed,
    PartiallyFailed,
}

/// Status event received from WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEvent {
    /// Execution/conversation ID
    pub execution_id: String,
    
    /// When this event occurred
    pub timestamp: DateTime<Utc>,
    
    /// Source that generated this event
    pub source: EventSource,
    
    /// The actual event data
    pub event: EventType,
}

/// Source of a status event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventSource {
    /// Event from the orchestrator
    Orchestrator,
    
    /// Event from a specific agent
    Agent { 
        agent_id: String, 
        agent_type: String 
    },
    
    /// Event from a tool being used by an agent
    Tool { 
        tool_name: String, 
        agent_id: String 
    },
    
    /// Event from workflow/DAG execution
    Workflow { 
        node_id: String, 
        wave: usize 
    },
    
    /// Event from human-in-the-loop system
    Hitl { 
        request_id: String 
    },
}

/// Types of events that can occur during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventType {
    /// Execution has started
    ExecutionStarted { 
        query: String 
    },
    
    /// Execution completed successfully
    ExecutionCompleted { 
        result: String 
    },
    
    /// Execution failed with an error
    ExecutionFailed { 
        error: String 
    },
    
    /// An agent has started working
    AgentStarted { 
        context_size: usize 
    },
    
    /// Agent is thinking/processing
    AgentThinking { 
        thought: String 
    },
    
    /// Agent has completed its task
    AgentCompleted { 
        result: String 
    },
    
    /// Agent failed to complete its task
    AgentFailed { 
        error: String 
    },
    
    /// A tool has started executing
    ToolStarted { 
        args: serde_json::Value 
    },
    
    /// Tool execution completed
    ToolCompleted { 
        result: serde_json::Value 
    },
    
    /// Tool execution failed
    ToolFailed { 
        error: String 
    },
    
    /// Human-in-the-loop approval requested
    HitlRequested { 
        task_description: String,
        risk_level: String,
    },
    
    /// Human-in-the-loop decision received
    HitlCompleted { 
        approved: bool,
        reason: Option<String>,
    },
    
    /// Workflow step started
    WorkflowStepStarted { 
        step_name: String 
    },
    
    /// Workflow step completed
    WorkflowStepCompleted { 
        step_name: String 
    },
}

/// History entry for display
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub entry_type: HistoryEntryType,
    pub conversation_id: String,
}

/// Type of history entry
#[derive(Debug, Clone)]
pub enum HistoryEntryType {
    Query { text: String },
    AgentActivity { agent_id: String, activity: String },
    ToolCall { tool_name: String, status: ToolCallStatus },
    RagRetrieval { query: String, results_count: usize },
    Result { text: String, success: bool },
    Error { message: String },
}

/// Status of a tool call
#[derive(Debug, Clone, PartialEq)]
pub enum ToolCallStatus {
    Started,
    Completed,
    Failed,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_conversation: None,
            conversation_history: VecDeque::new(),
            capabilities: None,
            connection_status: ConnectionStatus::Disconnected,
            ui_state: UiState::default(),
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            input_text: String::new(),
            input_cursor: 0,
            focus: FocusState::QueryInput,
            scroll_positions: ScrollPositions::default(),
            dimensions: UiDimensions::default(),
        }
    }
}

impl Default for ScrollPositions {
    fn default() -> Self {
        Self {
            history_scroll: 0,
            dag_scroll: 0,
        }
    }
}

impl Default for UiDimensions {
    fn default() -> Self {
        Self {
            terminal_width: 80,
            terminal_height: 24,
            history_width: 50,
            dag_width: 30,
        }
    }
}

impl Conversation {
    pub fn new(id: String, query: String) -> Self {
        Self {
            id,
            query,
            started_at: Utc::now(),
            status: ConversationStatus::Started,
            workflow: None,
            events: Vec::new(),
            result: None,
        }
    }
    
    pub fn add_event(&mut self, event: StatusEvent) {
        // Update conversation status based on event
        match &event.event {
            EventType::ExecutionStarted { .. } => {
                self.status = ConversationStatus::Planning;
            }
            EventType::WorkflowStepStarted { .. } => {
                self.status = ConversationStatus::Executing;
            }
            EventType::ExecutionCompleted { result } => {
                self.status = ConversationStatus::Completed;
                self.result = Some(result.clone());
            }
            EventType::ExecutionFailed { error } => {
                self.status = ConversationStatus::Failed { 
                    error: error.clone() 
                };
            }
            _ => {}
        }
        
        self.events.push(event);
    }
    
    /// Get display-friendly status text
    pub fn status_text(&self) -> &str {
        match &self.status {
            ConversationStatus::Started => "Started",
            ConversationStatus::Planning => "Planning",
            ConversationStatus::Executing => "Executing",
            ConversationStatus::Completed => "Completed",
            ConversationStatus::Failed { .. } => "Failed",
        }
    }
    
    /// Get the latest events for display
    pub fn recent_events(&self, limit: usize) -> &[StatusEvent] {
        let len = self.events.len();
        if len <= limit {
            &self.events
        } else {
            &self.events[len - limit..]
        }
    }
}