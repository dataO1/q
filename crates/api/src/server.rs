use ai_agent_common::*;
use axum::{Router, routing::post};
use std::sync::Arc;
use anyhow::Result;

use crate::routes::execute::AppState;

pub struct AcpServer {
}

impl AcpServer {
    pub fn new() -> Self {
        todo!("Initialize ACP server")
    }

    pub fn router(&self) -> Router<AppState> {
        Router::new()
            // .route("/execute", post(crate::routes::execute::execute_task))
            // .route("/stream/:task_id", axum::routing::get(crate::routes::stream::stream_status))
            // .route("/agents", axum::routing::get(crate::routes::agents::list_agents))
    }

    pub async fn run(self, addr: &str) -> Result<()> {
        todo!("Start Axum server")
    }
}
