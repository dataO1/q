//! ACP API service layer

use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use tracing::{debug, error, info, warn};

use crate::{
    client::{types::*, AcpClient},
};

/// Service for handling ACP API interactions
#[derive(Clone)]
pub struct ApiService {
    client: Arc<AcpClient>,
}

impl ApiService {
    /// Create new API service
    pub fn new(client: Arc<AcpClient>) -> Self {
        Self { client }
    }

    /// Retry an async operation with exponential backoff
    async fn retry_with_backoff<T, F, Fut>(&self, operation: F, operation_name: &str) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        const MAX_RETRIES: u32 = 3;
        const BASE_DELAY_MS: u64 = 100;

        let mut last_error = None;

        for attempt in 1..=MAX_RETRIES {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);

                    if attempt < MAX_RETRIES {
                        let delay = Duration::from_millis(BASE_DELAY_MS * 2_u64.pow(attempt - 1));
                        warn!(
                            "{} failed (attempt {}/{}), retrying in {:?}: {}",
                            operation_name, attempt, MAX_RETRIES, delay,
                            last_error.as_ref().unwrap()
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// Execute a query through the ACP API
    pub async fn execute_query(&self, query: String, subscription_id: String) -> Result<String> {
        info!("Executing query: {}", query);

        let query_clone = query.clone();
        let subscription_id_clone = subscription_id.clone();
        self.retry_with_backoff(
            move || {
                let query_clone = query_clone.clone();
                let subscription_id_clone = subscription_id_clone.clone();
                async move {
                // Create the proper request using generated API types
                let project_scope = crate::client::detect_project_scope().unwrap_or_else(|_| {
                    ProjectScope {
                        root: ".".to_string(),
                        current_file: None,
                        language_distribution: std::collections::HashMap::new(),
                    }
                });

                let request = QueryRequest {
                    query: query_clone.clone(),
                    project_scope,
                    subscription_id: subscription_id_clone.clone(),
                };

                // Use the generated client's query_task method (from operationId)
                match self.client.client().query_task(&request).await {
                    Ok(response) => {
                        let result = response.into_inner();
                        info!("Query executed successfully, subscription_id: {}", result.subscription_id);
                        Ok(result.subscription_id)
                    }
                    Err(e) => {
                        error!("Query execution failed: {}", e);
                        Err(e.into())
                    }
                }
                }
            },
            "Query execution"
        ).await
    }

    /// Create subscription
    pub async fn create_subscription(&self, client_id: String) -> Result<String> {
        info!("Creating subscription for client: {}", client_id);

        // Create subscription request using generated API types
        let request = SubscribeRequest {
            client_id: Some(client_id),
        };

        // Use the generated client's create_subscription method (from operationId)
        // The method takes the request body as a parameter directly
        match self.client.client().create_subscription(&request).await {
            Ok(response) => {
                let result = response.into_inner();
                info!("Subscription created: {}", result.subscription_id);
                Ok(result.subscription_id)
            }
            Err(e) => {
                error!("Subscription creation failed: {}", e);
                Err(e.into())
            }
        }
    }

    /// Get health status
    pub async fn get_health(&self) -> Result<HealthResponse> {
        debug!("Checking server health");

        // Use the generated client's health check method (as shown in client.rs)
        match self.client.client().health_check().await {
            Ok(response) => {
                let health = response.into_inner();
                debug!("Server health: {:?}", health);
                Ok(health)
            }
            Err(e) => {
                error!("Health check failed: {}", e);
                Err(e.into())
            }
        }
    }
}
