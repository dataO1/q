use ai_agent_common::*;
use ai_agent_orchestrator::OrchestratorSystem;
use axum::{Router, routing::post};
use std::sync::Arc;

use crate::routes::execute::AppState;

pub struct AcpServer {
    orchestrator: Arc<OrchestratorSystem>,
}

impl AcpServer {
    pub fn new(orchestrator: Arc<OrchestratorSystem>) -> Self {
        todo!("Initialize ACP server")
    }

    pub fn router(&self) -> Router<AppState> {
        Router::new()
            .route("/execute", post(crate::routes::execute::execute_task))
            .route("/stream/:task_id", axum::routing::get(crate::routes::stream::stream_status))
            .route("/agents", axum::routing::get(crate::routes::agents::list_agents))
    }

    pub async fn run(self, addr: &str) -> Result<()> {
        todo!("Start Axum server")
    }
}
