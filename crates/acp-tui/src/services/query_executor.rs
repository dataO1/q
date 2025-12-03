//! Query execution service with proper error handling and state management

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::{
    message::APIEvent,
    services::ApiService,
};

/// Service for handling query execution with proper async orchestration
#[derive(Clone)]
pub struct QueryExecutor {
    api_service: ApiService,
    sender: mpsc::UnboundedSender<APIEvent>,
}

impl QueryExecutor {
    /// Create new query executor
    pub fn new(api_service: ApiService, sender: mpsc::UnboundedSender<APIEvent>) -> Self {
        Self { api_service, sender }
    }

    /// Execute a query asynchronously
    pub async fn execute_query(&self, query: String, subscription_id: String) -> Result<()> {
        if query.trim().is_empty() {
            warn!("Attempted to execute empty query");
            return Ok(());
        }

        info!("Starting query execution: {}", query);

        // Send query execution started message
        let _ = self.sender.send(APIEvent::QueryExecutionStarted(query.clone()));

        match self.api_service.execute_query(query.clone(), subscription_id).await {
            Ok(conversation_id) => {
                info!("Query execution started with ID: {}", conversation_id);
                let _ = self.sender.send(APIEvent::QueryExecutionCompleted(conversation_id));
                Ok(())
            }
            Err(e) => {
                error!("Query execution failed: {}", e);
                let _ = self.sender.send(APIEvent::QueryExecutionFailed(e.to_string()));
                Err(e).context("Failed to execute query")
            }
        }
    }

    /// Validate query before execution
    pub fn validate_query(&self, query: &str) -> Result<()> {
        if query.trim().is_empty() {
            return Err(anyhow::anyhow!("Query cannot be empty"));
        }

        if query.len() > 10000 {
            return Err(anyhow::anyhow!("Query too long (max 10000 characters)"));
        }

        // Add more validation rules as needed
        Ok(())
    }

    /// Get query execution history
    pub fn get_query_history(&self) -> Vec<String> {
        // Query history persistence could be implemented with local storage
        // For now, return empty as this is not a critical feature for the TUI
        vec![]
    }

    /// Clear query history
    pub fn clear_query_history(&self) -> Result<()> {
        // Query history management - would clear local storage if implemented
        // For now, this is a no-op since we're not persisting history yet
        Ok(())
    }
}
