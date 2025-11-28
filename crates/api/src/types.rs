//! Type definitions for the ACP API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ai_agent_common::AgentType;

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

