use ai_agent_common::*;
use ai_agent_orchestrator::OrchestratorSystem;
use axum::{Json, extract::State};
use rig::completion::CompletionModel;

#[derive(serde::Deserialize)]
pub struct ExecuteRequest {
    pub query: String,
    pub conversation_id: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ExecuteResponse {
    pub task_id: String,
    pub stream_url: String,
}

pub async fn execute_task(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Json<ExecuteResponse> {
    todo!("Start task execution")
}

#[derive(Clone)]
pub struct AppState {
    pub orchestrator: std::sync::Arc<OrchestratorSystem>,
}
