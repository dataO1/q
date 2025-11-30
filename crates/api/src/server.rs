use ai_agent_common::SystemConfig;
use ai_agent_network::execution_manager::ExecutionManager;
use axum::{
    Router, 
    routing::{get, post},
    middleware::from_fn,
    response::Json,
};
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, instrument};
use tower_http::{
    trace::TraceLayer,
    cors::{CorsLayer, Any},
};

use crate::{
    routes::{
        query::query_task,
        stream::websocket_handler,
        agents::list_capabilities,
        subscribe::{create_subscription, get_subscription_status},
    },
    middleware::logging::logging_middleware,
    openapi::ApiDoc,
    types::HealthResponse,
};
use utoipa::OpenApi;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// The execution manager from agent-network crate
    pub execution_manager: Arc<ExecutionManager>,
    
    /// System configuration loaded at startup
    pub config: SystemConfig,
}

/// ACP Server for agent communication
pub struct AcpServer {
    state: AppState,
}

impl AcpServer {
    /// Create a new ACP server with system configuration loaded once at startup
    pub async fn new(config: SystemConfig) -> Result<Self> {
        info!("Initializing ACP server with config from {:?}", 
               std::env::current_dir().unwrap_or_default());
        
        // Create execution manager with full system config
        let execution_manager = ExecutionManager::new(config.clone()).await?;
        let execution_manager = Arc::new(execution_manager);
        
        let state = AppState {
            execution_manager,
            config, // Store entire config for server lifetime
        };
        
        Ok(Self { state })
    }

    /// Create the Axum router with all routes
    pub fn router(&self) -> Router {
        Router::new()
            // Core ACP endpoints
            .route("/query", post(query_task))
            .route("/stream/{subscription_id}", get(websocket_handler))
            
            // Subscription management
            .route("/subscribe", post(create_subscription))
            .route("/subscribe/{subscription_id}", get(get_subscription_status))
            
            // Discovery and health endpoints
            .route("/capabilities", get(list_capabilities))
            .route("/health", get(health_check))
            
            // OpenAPI spec endpoint
            .route("/api-doc/openapi.json", get(serve_openapi_spec))
            
            // Apply state and middleware
            .with_state(self.state.clone())
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods(Any)
            )
            .layer(from_fn(logging_middleware))
    }

    /// Start the ACP server using configuration
    #[instrument(skip(self))]
    pub async fn run(self) -> Result<()> {
        // Use ACP config from loaded system configuration
        let bind_addr = format!("{}:{}", 
            self.state.config.agent_network.acp.host,
            self.state.config.agent_network.acp.port
        );
        
        info!("Starting ACP server on {}", bind_addr);
        
        let app = self.router();
        let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
        
        info!("ACP server listening on http://{}", listener.local_addr()?);
        axum::serve(listener, app).await?;
        
        Ok(())
    }
    
    /// Get access to the application state (for testing)
    pub fn state(&self) -> &AppState {
        &self.state
    }
}

/// Health check endpoint
/// 
/// Returns the current status and health of the ACP API server.
/// Use this endpoint to verify server availability and operational status.
/// 
/// ## Response
/// 
/// Returns health information including:
/// - Server status ("healthy" indicates full functionality)
/// - Optional status message with additional details
/// - Timestamp when the check was performed
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Server is healthy", body = HealthResponse)
    )
)]
#[instrument]
pub async fn health_check() -> Json<HealthResponse> {
    use chrono::Utc;
    
    info!("Health check requested");
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        message: Some("ACP server is running".to_string()),
        timestamp: Utc::now(),
    })
}

/// Serve the OpenAPI specification
#[instrument]
pub async fn serve_openapi_spec() -> Json<utoipa::openapi::OpenApi> {
    info!("OpenAPI specification requested");
    Json(ApiDoc::openapi())
}

