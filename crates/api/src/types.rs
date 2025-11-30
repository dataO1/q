//! Type definitions for the ACP API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use ai_agent_common::AgentType;

// Re-export common types to avoid qualified references in OpenAPI
pub use ai_agent_common::{ProjectScope, StatusEvent, EventType, EventSource, ExecutionPlan, WaveInfo, TaskInfo};

/// Request to execute a query
/// 
/// Starts asynchronous query execution with the multi-agent system.
/// Requires a subscription_id obtained from the /subscribe endpoint first.
#[derive(Debug, Deserialize, ToSchema)]
pub struct QueryRequest {
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
    pub project_scope: ProjectScope,
    
    /// Subscription ID for streaming events
    /// 
    /// Must be obtained from POST /subscribe before executing the query.
    /// This ensures events are buffered and no progress is lost.
    #[schema(example = "sub_750e8400-e29b-41d4-a716-446655440123")]
    pub subscription_id: String,
}

/// Response when starting an execution
/// 
/// Returned immediately when a query execution starts. The actual processing
/// happens asynchronously in the background with events streamed to the subscription.
#[derive(Debug, Serialize, ToSchema)]
pub struct QueryResponse {
    /// Subscription ID for this execution
    /// 
    /// The same subscription_id from the request, confirming execution started.
    /// Use this with the WebSocket stream for real-time updates.
    #[schema(example = "sub_750e8400-e29b-41d4-a716-446655440123")]
    pub subscription_id: String,
    
    /// WebSocket URL path for streaming status updates
    /// 
    /// Connect to this WebSocket endpoint to receive real-time progress updates.
    /// The URL is relative to the API base URL.
    #[schema(example = "/stream/sub_750e8400-e29b-41d4-a716-446655440123")]
    pub stream_url: String,
    
    /// Current execution status
    /// 
    /// Always "started" for successful responses. Use the WebSocket stream
    /// for detailed progress updates.
    #[schema(example = "started")]
    pub status: String,
}

/// Request to create a subscription
/// 
/// Creates a subscription that will buffer events for future query execution.
/// Must be called before executing a query to ensure no events are lost.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubscribeRequest {
    /// Optional client identifier for tracking
    /// 
    /// Helps with debugging and metrics. If not provided, a unique
    /// client ID will be generated for this subscription.
    #[schema(example = "tui-client-1")]
    pub client_id: Option<String>,
}

/// Response when creating a subscription
/// 
/// Contains the information needed to connect to the WebSocket stream
/// and start receiving events for this subscription.
#[derive(Debug, Serialize, ToSchema)]
pub struct SubscribeResponse {
    /// Unique subscription ID for this client
    /// 
    /// Use this ID to connect to the WebSocket stream. Each subscription
    /// gets its own buffer of events and replay behavior.
    #[schema(example = "sub_750e8400-e29b-41d4-a716-446655440123")]
    pub subscription_id: String,
    
    /// WebSocket URL path for this subscription
    /// 
    /// Connect to this WebSocket endpoint to receive buffered and live events.
    /// The URL is relative to the API base URL.
    #[schema(example = "/stream/sub_750e8400-e29b-41d4-a716-446655440123")]
    pub stream_url: String,
    
    /// When this subscription expires
    /// 
    /// Subscriptions automatically expire after a timeout to prevent
    /// memory leaks. Connect before this time to avoid losing events.
    #[schema(example = "2024-01-01T12:05:00Z")]
    pub expires_at: DateTime<Utc>,
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

/// Subscription status information
/// 
/// Contains information about an active subscription including its state
/// and connection details.
#[derive(Debug, Serialize, ToSchema)]
pub struct SubscriptionStatus {
    /// Unique subscription ID
    #[schema(example = "sub_750e8400-e29b-41d4-a716-446655440123")]
    pub subscription_id: String,
    
    /// Current subscription state
    /// 
    /// - "waiting": Subscription created, waiting for query execution
    /// - "active": Query executing, buffering events, WebSocket not connected
    /// - "connected": WebSocket is connected and streaming events
    /// - "completed": Query execution finished
    /// - "expired": Subscription has expired and is no longer valid
    #[schema(example = "waiting")]
    pub status: String,
    
    /// When this subscription was created
    pub created_at: DateTime<Utc>,
    
    /// When this subscription expires
    pub expires_at: DateTime<Utc>,
    
    /// Number of events buffered for this subscription
    #[schema(example = 5)]
    pub buffered_events: usize,
    
    /// Whether a WebSocket is currently connected
    pub connected: bool,
    
    /// Optional client identifier
    #[schema(example = "tui-client-1")]
    pub client_id: Option<String>,
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

// HITL (Human-in-the-Loop) API types

/// HITL approval request details
/// 
/// Contains all information needed for a human to review and approve/reject
/// an agent's proposed action.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HitlApprovalRequest {
    /// Unique identifier for this approval request
    #[schema(example = "hitl_550e8400-e29b-41d4-a716-446655440001")]
    pub request_id: String,
    
    /// Task ID that triggered this approval request
    #[schema(example = "Coding-4c7b3d00-81a2-43d2-aef6-2850ea6b5fad")]
    pub task_id: String,
    
    /// Agent ID that made the request
    #[schema(example = "coding-1")]
    pub agent_id: String,
    
    /// Type of agent making the request
    pub agent_type: AgentType,
    
    /// Risk level assessment
    /// 
    /// Determines urgency and required approval level.
    #[schema(example = "High")]
    pub risk_level: String,
    
    /// Summary of what the agent wants to do
    /// 
    /// Human-readable description of the proposed action.
    #[schema(example = "Create database migration to add user authentication tables")]
    pub proposed_action: String,
    
    /// Detailed changes the agent wants to make
    pub proposed_changes: Vec<ProposedChange>,
    
    /// Additional context about why this action is needed
    #[schema(example = "User requested authentication system. This migration creates the necessary database schema.")]
    pub context: String,
    
    /// When this request was created
    pub timestamp: DateTime<Utc>,
}

/// A specific change proposed by an agent
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProposedChange {
    /// File path for file operations (None for non-file operations)
    #[schema(example = "migrations/001_create_users.sql")]
    pub file_path: Option<String>,
    
    /// Type of change being proposed
    pub change_type: ChangeType,
    
    /// Content of the change (new content for creates, full content for modifies)
    #[schema(example = "CREATE TABLE users (id SERIAL PRIMARY KEY, email VARCHAR(255) UNIQUE, created_at TIMESTAMP DEFAULT NOW());")]
    pub content: String,
    
    /// Optional diff showing what changed (for modify operations)
    pub diff: Option<String>,
}

/// Type of change an agent wants to make
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum ChangeType {
    /// Create a new file or resource
    Create,
    /// Modify existing content
    Modify,
    /// Delete a file or resource
    Delete,
    /// Execute a command
    Execute,
}

/// Request to make a decision on a HITL approval
#[derive(Debug, Deserialize, ToSchema)]
pub struct HitlDecisionRequest {
    /// The approval request ID being decided on
    #[schema(example = "hitl_550e8400-e29b-41d4-a716-446655440001")]
    pub request_id: String,
    
    /// Decision made by the human reviewer
    pub decision: HitlDecision,
    
    /// Modified content if decision is Modify
    /// 
    /// Contains the human's edited version of the proposed changes.
    pub modified_content: Option<String>,
    
    /// Optional reason for the decision
    /// 
    /// Particularly useful for rejections to help the agent understand.
    #[schema(example = "Please use more descriptive column names")]
    pub reason: Option<String>,
}

/// Human decision on an approval request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum HitlDecision {
    /// Approve the proposed action as-is
    Approve,
    /// Reject the proposed action
    Reject,
    /// Approve with modifications (requires modified_content)
    Modify,
}

/// Response after submitting a HITL decision
#[derive(Debug, Serialize, ToSchema)]
pub struct HitlDecisionResponse {
    /// The request ID that was decided on
    pub request_id: String,
    
    /// Confirmation of the decision made
    pub decision: HitlDecision,
    
    /// When the decision was processed
    pub processed_at: DateTime<Utc>,
    
    /// Success message
    #[schema(example = "Decision processed successfully")]
    pub message: String,
}

/// List of pending HITL requests
#[derive(Debug, Serialize, ToSchema)]
pub struct HitlPendingResponse {
    /// List of requests awaiting human decision
    pub requests: Vec<HitlApprovalRequest>,
    
    /// Total number of pending requests
    pub count: usize,
}

/// Detailed view of a specific HITL request
#[derive(Debug, Serialize, ToSchema)]
pub struct HitlRequestDetails {
    /// The full approval request
    pub request: HitlApprovalRequest,
    
    /// Additional metadata about the request
    pub metadata: HitlMetadata,
}

/// Additional metadata about a HITL request
#[derive(Debug, Serialize, ToSchema)]
pub struct HitlMetadata {
    /// Which execution this request belongs to
    pub execution_id: String,
    
    /// Current status of the request
    #[schema(example = "pending")]
    pub status: String,
    
    /// How long this request has been pending
    pub pending_duration_ms: u64,
    
    /// Related task context
    pub task_context: TaskContext,
}

/// Context about the task that triggered a HITL request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskContext {
    /// Task description
    #[schema(example = "Implement database migration for user authentication")]
    pub description: String,
    
    /// Wave this task is part of
    pub wave_index: u64,
    
    /// Dependencies this task has
    pub dependencies: Vec<String>,
}

