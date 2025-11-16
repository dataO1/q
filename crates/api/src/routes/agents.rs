use axum::{response::Json, Extension};
use serde_json::json;
use std::sync::Arc;

// pub async fn list_agents(
//     Extension(orchestrator): Extension<Arc<OrchestratorSystem>>,
// ) -> Json<serde_json::Value> {
//     let agents = orchestrator.list_agents().await;
//     Json(json!({ "agents": agents }))
// }
