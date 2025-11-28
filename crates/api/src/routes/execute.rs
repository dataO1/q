use axum::{http::StatusCode, Json, extract::State, response::Result};
use ai_agent_common::ConversationId;
use serde_json::json;
use tracing::{info, error, instrument};
use chrono::Utc;
use crate::{
    types::{ExecuteRequest, ExecuteResponse, ErrorResponse},
    server::AppState,
};

/// Execute a query asynchronously
/// 
/// Starts background execution of a user query and returns immediately with a conversation_id.
/// Real-time progress updates are available via WebSocket streaming at the returned stream_url.
/// 
/// ## Behavior
/// 
/// - **Immediate Response**: Returns conversation_id without waiting for completion
/// - **Background Execution**: Query processing happens asynchronously  
/// - **Streaming Updates**: Connect to WebSocket for real-time progress
/// - **Conversation Context**: Reuse conversation_id for related queries
/// 
/// ## Project Scope
/// 
/// The `project_scope` field must be populated by the client with:
/// - Project root directory path
/// - Detected programming languages  
/// - Key files and their purposes
/// - Active development areas
/// 
/// This context allows agents to understand the codebase structure and provide
/// relevant assistance with appropriate tools.
/// 
/// ## WebSocket Events
/// 
/// After calling this endpoint, connect to the WebSocket stream to receive:
/// 
/// 1. `ExecutionStarted` - Processing has begun
/// 2. `AgentStarted` - An agent has been assigned
/// 3. `WorkflowStepStarted/Completed` - Step-by-step progress  
/// 4. `AgentThinking` - Intermediate thoughts (optional)
/// 5. `AgentCompleted` - Agent finished with results
/// 6. `ExecutionCompleted` - Final results available
/// 
/// ## Error Handling
/// 
/// - API errors return `ErrorResponse` immediately
/// - Execution errors are streamed as `ExecutionFailed` events
/// - WebSocket disconnections don't affect background processing
/// 
/// ## Example Usage
/// 
/// ```bash
/// # Start execution
/// curl -X POST /execute \
///   -H "Content-Type: application/json" \
///   -d '{"query": "Analyze the auth module", "project_scope": {...}}'
/// 
/// # Connect to stream  
/// wscat -c "ws://localhost:3000/stream/{conversation_id}"
/// ```
#[utoipa::path(
    post,
    path = "/execute", 
    tag = "execution",
    summary = "Execute query asynchronously",
    description = "Start background execution of a user query with real-time streaming updates",
    request_body(
        content = ExecuteRequest,
        description = "Query and project context for execution",
        example = json!({
            "query": "Analyze the authentication module and suggest improvements", 
            "project_scope": {
                "root": "/home/user/my-project",
                "languages": ["rust", "typescript"],
                "frameworks": ["axum", "react"],
                "key_files": [
                    {"path": "src/auth.rs", "purpose": "Authentication logic"},
                    {"path": "src/main.rs", "purpose": "Application entry point"}
                ]
            },
            "conversation_id": "550e8400-e29b-41d4-a716-446655440000"
        })
    ),
    responses(
        (status = 200, description = "Execution started successfully", body = ExecuteResponse,
         example = json!({
             "execution_id": "550e8400-e29b-41d4-a716-446655440000",
             "stream_url": "/stream/550e8400-e29b-41d4-a716-446655440000", 
             "status": "started"
         })),
        (status = 500, description = "Failed to start execution", body = ErrorResponse,
         example = json!({
             "error": "Failed to start execution: Invalid project scope",
             "code": "EXECUTION_START_FAILED",
             "timestamp": "2024-01-15T14:30:00Z"
         }))
    )
)]
#[instrument(skip(state))]
pub async fn execute_task(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Starting execution for query: {} with project scope: {:?}", 
          req.query, req.project_scope);

    // Use project scope provided by client
    let project_scope = req.project_scope;

    // Create conversation ID  
    let conversation_id = req.conversation_id
        .map(|id| ConversationId::from_string(id))
        .unwrap_or_else(|| ConversationId::new());

    // Execute query through execution manager (returns immediately, runs async)
    let result = state.execution_manager.execute_query(
        &req.query,
        project_scope,
        conversation_id.clone()
    ).await;

    match result {
        Ok(conversation_id_str) => {
            info!("Execution started for conversation {}", conversation_id_str);
        }
        Err(e) => {
            error!("Failed to start execution: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to start execution: {}", e),
                    code: Some("EXECUTION_START_FAILED".to_string()),
                    timestamp: Utc::now(),
                })
            ));
        }
    }

    let conversation_id_str = conversation_id.to_string();
    let response = ExecuteResponse {
        execution_id: conversation_id_str.clone(),
        stream_url: format!("/stream/{}", conversation_id_str),
        status: "started".to_string(),
    };

    info!("Execution started successfully for conversation {}", conversation_id_str);
    Ok(Json(response))
}
