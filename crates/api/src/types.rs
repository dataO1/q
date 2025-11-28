//! Type definitions for the ACP API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ai_agent_common::AgentType;

/// Real-time status event for WebSocket streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEvent {
    /// Unique identifier for this execution
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
        agent_type: AgentType 
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
    
    /// Agent is thinking/processing (streaming thoughts)
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
    
    /// Human input/approval is requested
    HITLRequested { 
        prompt: String 
    },
    
    /// Human has responded to HITL request
    HITLCompleted { 
        response: String 
    },
    
    /// Workflow progress update
    WorkflowProgress { 
        current_wave: usize, 
        total_waves: usize 
    },
}

/// Request to execute a query
#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    /// The user's query/request
    pub query: String,
    
    /// Project scope detected by the client
    pub project_scope: ai_agent_common::ProjectScope,
    
    /// Optional conversation ID for maintaining context
    pub conversation_id: Option<String>,
}

/// Response when starting an execution
#[derive(Debug, Serialize)]
pub struct ExecuteResponse {
    /// Unique ID for this execution
    pub execution_id: String,
    
    /// WebSocket URL to stream status updates
    pub stream_url: String,
    
    /// Current status
    pub status: String,
}

/// System capabilities response
#[derive(Debug, Serialize)]
pub struct CapabilitiesResponse {
    /// Available agent types
    pub agents: Vec<AgentCapability>,
    
    /// Supported features
    pub features: Vec<String>,
    
    /// API version
    pub version: String,
}

/// Information about an available agent type
#[derive(Debug, Serialize)]
pub struct AgentCapability {
    /// Type of agent
    pub agent_type: AgentType,
    
    /// Human-readable description
    pub description: String,
    
    /// Tools this agent can use
    pub tools: Vec<String>,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Health status
    pub status: String,
    
    /// Optional additional information
    pub message: Option<String>,
    
    /// Timestamp of health check
    pub timestamp: DateTime<Utc>,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    
    /// Optional error code
    pub code: Option<String>,
    
    /// Timestamp of error
    pub timestamp: DateTime<Utc>,
}

impl StatusEvent {
    /// Create a new status event
    pub fn new(execution_id: String, source: EventSource, event: EventType) -> Self {
        Self {
            execution_id,
            timestamp: Utc::now(),
            source,
            event,
        }
    }
}