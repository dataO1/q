//! ACP (Agent Communication Protocol) server

use crate::{
    orchestrator::Orchestrator,
    error::Result,
};
use axum::{
    extract::State,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AcpServer {
    orchestrator: Arc<RwLock<Orchestrator>>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct ExecuteResponse {
    pub result: String,
    pub success: bool,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
}

pub async fn start_server(orchestrator: Orchestrator) -> Result<()> {
    let orchestrator = Arc::new(RwLock::new(orchestrator));

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/execute", post(execute_query))
        .route("/status", get(status_stream))
        .with_state(orchestrator);

    // TODO: Week 7 - Complete ACP server implementation
    // - Add WebSocket support for streaming
    // - Implement all API endpoints
    // - Add authentication/authorization
    // - Handle HITL approval queue

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    let addr = listener.local_addr()?;
    tracing::info!("ACP server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
    })
}

async fn execute_query(
    State(orchestrator): State<Arc<RwLock<Orchestrator>>>,
    Json(request): Json<ExecuteRequest>,
) -> Json<ExecuteResponse> {
    let orchestrator = orchestrator.read().await;

    match orchestrator.execute_query(&request.query).await {
        Ok(result) => Json(ExecuteResponse {
            result,
            success: true,
        }),
        Err(e) => Json(ExecuteResponse {
            result: format!("Error: {}", e),
            success: false,
        }),
    }
}

async fn status_stream() -> Json<serde_json::Value> {
    // TODO: Week 7 - Implement proper WebSocket streaming
    Json(serde_json::json!({
        "message": "Status streaming not yet implemented"
    }))
}
