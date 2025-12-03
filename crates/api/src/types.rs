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
