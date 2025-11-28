use axum::{http::StatusCode, Json, extract::State, response::Result};
use ai_agent_common::ConversationId;
use tracing::{info, error, instrument};
use chrono::Utc;
use crate::{
    types::{ExecuteRequest, ExecuteResponse, ErrorResponse},
    server::AppState,
};

/// Execute a user query through the orchestrator
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
