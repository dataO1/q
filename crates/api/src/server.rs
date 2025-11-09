use ai_agent_common::*;
use axum::{Router, routing::post};
use std::sync::Arc;

pub struct AcpServer {
    orchestrator: Arc<crate::orchestrator::OrchestratorSystem>,
}

impl AcpServer {
    pub fn new(orchestrator: Arc<crate::orchestrator::OrchestratorSystem>) -> Self {
        todo!("Initialize ACP server")
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/execute", post(crate::routes::execute::execute_task))
            .route("/stream/:task_id", axum::routing::get(crate::routes::stream::stream_status))
            .route("/agents", axum::routing::get(crate::routes::agents::list_agents))
    }

    pub async fn run(self, addr: &str) -> Result<()> {
        todo!("Start Axum server")
    }
}
