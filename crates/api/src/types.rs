//! Type definitions for the ACP API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use ai_agent_common::AgentType;

/// Request to execute a query
/// 
/// Starts asynchronous query execution with the multi-agent system.
/// Returns immediately with a conversation_id for tracking progress.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ExecuteRequest {
    /// The user's natural language query or request
    /// 
    /// Can be any natural language instruction for the agents to process.
    /// Examples: "Analyze this codebase", "Fix the authentication bug", 
    /// "Add error handling to the API endpoints"
    #[schema(example = "Analyze the authentication module and suggest improvements")]
    pub query: String,
    
    /// Project context detected by the client
    /// 
    /// Must be detected client-side and includes project root, languages,
    /// key files, and other contextual information needed by agents.
    /// This determines which tools and approaches agents can use.
    #[schema(value_type = Object, example = json!({
        "root": "/home/user/my-project",
        "languages": ["rust", "typescript"],
        "frameworks": ["axum", "react"],
        "key_files": [
            {"path": "src/main.rs", "purpose": "Application entry point"},
            {"path": "src/auth.rs", "purpose": "Authentication logic"}
        ]
    }))]
    pub project_scope: ai_agent_common::ProjectScope,
    
    /// Optional conversation ID for maintaining context across multiple queries
    /// 
    /// If provided, this query will be part of an existing conversation.
    /// If not provided, a new conversation will be created.
    /// Use this to maintain context across related queries.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub conversation_id: Option<String>,
}

/// Response when starting an execution
/// 
/// Returned immediately when a query execution starts. The actual processing
/// happens asynchronously in the background.
#[derive(Debug, Serialize, ToSchema)]
pub struct ExecuteResponse {
    /// Conversation ID for tracking this execution (same as conversation_id)
    /// 
    /// Use this ID to subscribe to the WebSocket stream for real-time updates.
    /// This is the conversation identifier, not just a single execution.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub execution_id: String,
    
    /// WebSocket URL path for streaming status updates
    /// 
    /// Connect to this WebSocket endpoint to receive real-time progress updates.
    /// The URL is relative to the API base URL.
    #[schema(example = "/stream/550e8400-e29b-41d4-a716-446655440000")]
    pub stream_url: String,
    
    /// Current execution status
    /// 
    /// Always "started" for successful responses. Use the WebSocket stream
    /// for detailed progress updates.
    #[schema(example = "started")]
    pub status: String,
}

/// System capabilities response
/// 
/// Information about available agents, features, and API capabilities.
/// Use this for discovery and capability negotiation.
#[derive(Debug, Serialize, ToSchema)]
pub struct CapabilitiesResponse {
    /// Available agent types and their capabilities
    pub agents: Vec<AgentCapability>,
    
    /// Supported API features
    /// 
    /// List of feature flags indicating what the API supports.
    /// Examples: "streaming", "multi_agent", "tool_execution", "hitl"
    #[schema(example = json!(["streaming", "multi_agent", "tool_execution"]))]
    pub features: Vec<String>,
    
    /// API version string
    #[schema(example = "1.0.0")]
    pub version: String,
}

/// Information about an available agent type
/// 
/// Describes the capabilities of each agent type available in the system.
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentCapability {
    /// Type identifier for this agent
    /// 
    /// Used in routing queries to appropriate agents based on task type.
    #[schema(example = "Coding")]
    pub agent_type: AgentType,
    
    /// Human-readable description of agent capabilities
    /// 
    /// Explains what this agent type is designed to handle.
    #[schema(example = "Analyzes code, implements features, fixes bugs, and provides programming assistance")]
    pub description: String,
    
    /// List of tools this agent can use
    /// 
    /// Tool names that this agent type has access to for completing tasks.
    #[schema(example = json!(["file_reader", "file_writer", "lsp_client", "test_runner"]))]
    pub tools: Vec<String>,
}

/// Health check response
/// 
/// Indicates the current health and status of the ACP API server.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Current health status
    /// 
    /// "healthy" indicates the API is fully operational.
    /// Other values may indicate degraded performance or issues.
    #[schema(example = "healthy")]
    pub status: String,
    
    /// Optional additional health information
    /// 
    /// May contain details about system state, resource usage, or warnings.
    #[schema(example = "All systems operational")]
    pub message: Option<String>,
    
    /// Timestamp when health check was performed
    pub timestamp: DateTime<Utc>,
}

/// Error response
/// 
/// Returned when an API request fails. Contains error details and context.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Human-readable error message
    /// 
    /// Describes what went wrong and may include suggestions for resolution.
    #[schema(example = "Failed to start execution: Invalid project scope")]
    pub error: String,
    
    /// Optional machine-readable error code
    /// 
    /// Standardized error codes for programmatic error handling.
    /// Examples: "EXECUTION_START_FAILED", "INVALID_PROJECT_SCOPE", "AGENT_UNAVAILABLE"
    #[schema(example = "EXECUTION_START_FAILED")]
    pub code: Option<String>,
    
    /// Timestamp when error occurred
    pub timestamp: DateTime<Utc>,
}

