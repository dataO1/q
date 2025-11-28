use ai_agent_common::SystemConfig;
use ai_agent_network::orchestrator::Orchestrator;
use axum::{
    Router, 
    routing::{get, post},
    middleware::from_fn,
};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use anyhow::Result;
use tracing::{info, instrument};
use tower_http::{
    trace::TraceLayer,
    cors::{CorsLayer, Any},
};

use crate::{
    types::StatusEvent,
    routes::{
        execute::execute_task,
        stream::websocket_handler,
        agents::list_capabilities,
    },
    middleware::logging::logging_middleware,
};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// The orchestrator from agent-network crate
    pub orchestrator: Arc<RwLock<Orchestrator>>,
    
    /// Broadcast channel for status events
    pub status_broadcaster: broadcast::Sender<StatusEvent>,
    
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
        
        // Create orchestrator with full system config
        let orchestrator = Orchestrator::new(config.clone()).await?;
        let orchestrator = Arc::new(RwLock::new(orchestrator));
        
        // Create broadcast channel for status events (capacity: 1000)
        let (status_broadcaster, _) = broadcast::channel(1000);
        
        let state = AppState {
            orchestrator,
            status_broadcaster,
            config, // Store entire config for server lifetime
        };
        
        Ok(Self { state })
    }

    /// Create the Axum router with all routes
    pub fn router(&self) -> Router {
        Router::new()
            // Core ACP endpoints
            .route("/execute", post(execute_task))
            .route("/stream/:execution_id", get(websocket_handler))
            
            // Discovery and health endpoints
            .route("/capabilities", get(list_capabilities))
            .route("/health", get(health_check))
            
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
async fn health_check() -> axum::Json<crate::types::HealthResponse> {
    use chrono::Utc;
    
    axum::Json(crate::types::HealthResponse {
        status: "healthy".to_string(),
        message: Some("ACP server is running".to_string()),
        timestamp: Utc::now(),
    })
}
