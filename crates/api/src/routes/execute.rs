use axum::{http::StatusCode, Json, extract::State, response::Result};
use ai_agent_common::ConversationId;
use uuid::Uuid;
use tracing::{info, warn, error, instrument};
use chrono::Utc;
use crate::{
    types::{ExecuteRequest, ExecuteResponse, ErrorResponse, StatusEvent, EventSource, EventType},
    server::AppState,
};

/// Execute a user query through the orchestrator
#[instrument(skip(state))]
pub async fn execute_task(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let execution_id = Uuid::new_v4().to_string();
    info!("Starting execution {} for query: {} with project scope: {:?}", 
          execution_id, req.query, req.project_scope);

    // Use project scope provided by client
    let project_scope = req.project_scope;

    // Create conversation ID
    let conversation_id = req.conversation_id
        .map(|id| ConversationId::from_string(id))
        .unwrap_or_else(|| ConversationId::new());

    // Send execution started event
    let start_event = StatusEvent::new(
        execution_id.clone(),
        EventSource::Orchestrator,
        EventType::ExecutionStarted { 
            query: req.query.clone() 
        },
    );
    
    if let Err(e) = state.status_broadcaster.send(start_event) {
        warn!("Failed to broadcast execution start event: {}", e);
    }

    // Start execution in background task
    let orchestrator = state.orchestrator.clone();
    let broadcaster = state.status_broadcaster.clone();
    let exec_id_clone = execution_id.clone();
    let query_clone = req.query.clone();
    
    tokio::spawn(async move {
        info!("Background execution task started for {}", exec_id_clone);
        
        // Execute query through orchestrator
        let orchestrator_guard = orchestrator.read().await;
        let result = orchestrator_guard.execute_query(
            &query_clone,
            project_scope,
            conversation_id
        ).await;
        
        // Send completion/failure event
        let completion_event = match result {
            Ok(result) => {
                info!("Execution {} completed successfully", exec_id_clone);
                StatusEvent::new(
                    exec_id_clone.clone(),
                    EventSource::Orchestrator,
                    EventType::ExecutionCompleted { result },
                )
            }
            Err(e) => {
                error!("Execution {} failed: {}", exec_id_clone, e);
                StatusEvent::new(
                    exec_id_clone.clone(),
                    EventSource::Orchestrator,
                    EventType::ExecutionFailed { 
                        error: e.to_string() 
                    },
                )
            }
        };
        
        if let Err(e) = broadcaster.send(completion_event) {
            warn!("Failed to broadcast execution completion event: {}", e);
        }
        
        info!("Background execution task finished for {}", exec_id_clone);
    });

    let response = ExecuteResponse {
        execution_id: execution_id.clone(),
        stream_url: format!("/stream/{}", execution_id),
        status: "started".to_string(),
    };

    info!("Execution {} started successfully", execution_id);
    Ok(Json(response))
}
