//! ACP (Agent Communication Protocol) server

use crate::{
    error::AgentNetworkResult, orchestrator::Orchestrator
};
use ai_agent_common::{ConversationId, ProjectScope, SystemConfig};
use ai_agent_rag::context_manager::ContextManager;
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
    pub cwd: String,
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

pub async fn start_server(config: SystemConfig) -> AgentNetworkResult<()> {

    let orchestrator = Orchestrator::new(config.clone()).await?;
    let orchestrator = std::sync::Arc::new(tokio::sync::RwLock::new(orchestrator));


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
    let uri = format!("{}:{}", config.agent_network.acp.host,  config.agent_network.acp.port);

    let listener = tokio::net::TcpListener::bind(uri).await?;
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
    if let Ok(project_scope) = ContextManager::detect_project_scope(request.cwd).await{
        match orchestrator.execute_query(&request.query, project_scope,ConversationId::new()).await {
            Ok(result) => Json(ExecuteResponse {
                result,
                success: true,
            }),
            Err(e) => Json(ExecuteResponse {
                result: format!("Error: {}", e),
                success: false,
            }),
        }
    }else{
            Json(ExecuteResponse {
                result: format!("Error: Failed to detect project scope"),
                success: false,
            })
    }
}

async fn status_stream() -> Json<serde_json::Value> {
    // TODO: Week 7 - Implement proper WebSocket streaming
    Json(serde_json::json!({
        "message": "Status streaming not yet implemented"
    }))
}
