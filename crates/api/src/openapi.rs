//! OpenAPI Specification Configuration
//!
//! This module defines the OpenAPI specification for the ACP (Agent Communication Protocol) API.
//! The specification is automatically generated from Rust types and route handlers using utoipa.

use utoipa::OpenApi;
use serde_json::json;
use crate::types::*;

/// OpenAPI specification for the ACP API
#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Agent Communication Protocol (ACP) API",
        description = "
# ACP API Documentation

REST API for multi-agent orchestration with real-time streaming capabilities.

## Overview

This API implements the Agent Communication Protocol (ACP) standard for coordinating
multiple AI agents in complex workflows. Key features:

- **Asynchronous Execution**: Queries return immediately with streaming updates
- **Conversation-based**: Multiple queries can be grouped in conversations
- **Real-time Streaming**: WebSocket events for live progress updates
- **Multi-agent Coordination**: Automatic task decomposition and agent routing

## Usage Pattern

1. **Execute Query**: POST `/query` with query and project context
2. **Get conversation_id**: Response contains ID for tracking
3. **Connect WebSocket**: Connect to `/stream/{conversation_id}` for updates
4. **Receive Events**: Get real-time progress as agents work
5. **Final Result**: ExecutionCompleted event contains final output

## Event Flow

```
ExecutionStarted → Query Analysis → Task Decomposition →
AgentStarted → WorkflowStepStarted → [AgentThinking*] →
WorkflowStepCompleted → AgentCompleted → ExecutionCompleted
```

## Project Scope

The `project_scope` field must be detected client-side and includes:
- Root directory path
- Programming languages detected
- Key files and their purposes
- Active development areas

This context allows agents to understand the codebase and provide relevant assistance.

## Error Handling

- API errors return ErrorResponse with details
- Execution errors are streamed as ExecutionFailed events
- Agent errors are streamed as AgentFailed events
- WebSocket disconnections don't affect background execution

## Standards Compliance

This API follows the [Agent Communication Protocol](https://agentcommunicationprotocol.dev)
standard for agent interoperability and communication.
        ",
        version = "1.0.0",
        contact(
            name = "ACP Team",
            url = "https://agentcommunicationprotocol.dev"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    paths(
        crate::routes::query::query_task,
        crate::routes::agents::list_capabilities,
        crate::routes::subscribe::create_subscription,
        crate::routes::subscribe::get_subscription_status,
        crate::server::health_check
    ),
    components(schemas(
        QueryRequest,
        QueryResponse,
        SubscribeRequest,
        SubscribeResponse,
        SubscriptionStatus,
        CapabilitiesResponse,
        AgentCapability,
        HealthResponse,
        ErrorResponse,
        HitlRequest,
        HitlMetadata,
        HitlPreview,
        // Common types
        crate::types::ProjectScope,
        crate::types::StatusEvent,
        crate::types::EventType,
        crate::types::EventSource,
        crate::types::ExecutionPlan,
        crate::types::WaveInfo,
        crate::types::TaskInfo,
        ai_agent_common::AgentType
    )),
    tags(
        (name = "query", description = "Query execution endpoints"),
        (name = "discovery", description = "Agent capability discovery"),
        (name = "health", description = "System health and status"),
        (name = "streaming", description = "Real-time status streaming (WebSocket)")
    ),
    external_docs(
        url = "https://agentcommunicationprotocol.dev/docs",
        description = "ACP Protocol Documentation"
    )
)]
pub struct ApiDoc;

/// Example StatusEvent for documentation
#[derive(utoipa::ToSchema)]
#[schema(
    title = "StatusEvent",
    example = json!({
        "conversation_id": "550e8400-e29b-41d4-a716-446655440000",
        "timestamp": "2024-01-15T14:30:00Z",
        "source": {
            "type": "agent",
            "agent_id": "coding-agent-1",
            "agent_type": "Coding"
        },
        "event": {
            "type": "agent_started",
            "context_size": 1247
        }
    })
)]
pub struct StatusEventDoc {
    /// Conversation/execution ID
    pub conversation_id: String,
    /// Event timestamp
    pub timestamp: String,
    /// Event source information
    pub source: EventSourceDoc,
    /// Event type and data
    pub event: EventTypeDoc,
}

/// Event source documentation
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
enum EventSourceDoc {
    /// Orchestrator events
    #[schema(example = json!({"type": "orchestrator"}))]
    Orchestrator,
    /// Agent events
    #[schema(example = json!({"type": "agent", "agent_id": "coding-agent-1", "agent_type": "Coding"}))]
    Agent { agent_id: String, agent_type: String },
}

/// Event type documentation
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
enum EventTypeDoc {
    /// Execution started
    #[schema(example = json!({"type": "execution_started", "query": "Analyze the code"}))]
    ExecutionStarted { query: String },
    /// Execution completed
    #[schema(example = json!({"type": "execution_completed", "result": "Analysis complete"}))]
    ExecutionCompleted { result: String },
    /// Agent started working
    #[schema(example = json!({"type": "agent_started", "context_size": 1247}))]
    AgentStarted { context_size: usize },
    /// Workflow step started
    #[schema(example = json!({"type": "workflow_step_started", "step_name": "Code Analysis"}))]
    WorkflowStepStarted { step_name: String },
    /// Agent completed
    #[schema(example = json!({"type": "agent_completed", "result": "Found 3 issues"}))]
    AgentCompleted { result: String },
}
