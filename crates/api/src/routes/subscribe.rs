//! Subscription management endpoints
//!
//! This module handles the creation and management of event stream subscriptions.
//! The two-phase subscription model prevents race conditions between query 
//! execution and WebSocket connection.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use tracing::{error, info, instrument, warn};
use crate::{server::AppState, types::*};

/// Create a subscription for query execution and event streaming
///
/// Creates a subscription that will buffer events for future query execution.
/// Must be called before executing a query to ensure no events are lost.
///
/// ## Client ID and Resumption
///
/// Provide a stable `client_id` (hardware-based) to enable reconnection:
/// - If `client_id` already has an active subscription, it will be resumed
/// - Otherwise, a new subscription is created
/// - Disconnected subscriptions are kept alive for 30 minutes for reconnection
/// 
/// **Client ID Generation (TUI should implement):**
/// ```rust
/// // Stable hardware-based ID using MAC address or similar
/// use sha2::{Sha256, Digest};
/// let client_id = format!("client_{}", 
///     hex::encode(&Sha256::digest(get_primary_mac_address().as_bytes())[..8])
/// );
/// ```
///
/// ## Usage Pattern
///
/// 1. Create subscription with `POST /subscribe` to get subscription_id
/// 2. Execute query with `POST /query` using subscription_id
/// 3. Connect WebSocket to `/stream/{subscription_id}` to receive events
///
/// ## Buffering Behavior
///
/// - Events are buffered from query execution start
/// - Buffered events are replayed immediately upon WebSocket connection
/// - Live events continue streaming after replay completes
/// - Subscriptions expire after 5 minutes, or 30 minutes if inactive
/// - WebSocket disconnection doesn't end subscription (enables reconnection)
///
/// ## Error Cases
///
/// - 500: Internal server error during subscription creation
#[utoipa::path(
    post,
    path = "/subscribe",
    request_body = SubscribeRequest,
    responses(
        (status = 200, description = "Subscription created successfully", body = SubscribeResponse),
        (status = 404, description = "Conversation not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "streaming"
)]
#[instrument(skip(state), fields(client_id = ?req.client_id))]
pub async fn create_subscription(
    State(state): State<AppState>,
    Json(req): Json<SubscribeRequest>,
) -> Result<Json<SubscribeResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        client_id = ?req.client_id,
        "Creating stream subscription"
    );

    // Create subscription through execution manager
    let subscription = match state.execution_manager
        .create_subscription(req.client_id.clone())
        .await
    {
        Ok(sub) => sub,
        Err(e) => {
            error!(
                error = %e,
                client_id = ?req.client_id,
                "Failed to create subscription"
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create subscription: {}", e),
                    code: Some("SUBSCRIPTION_CREATION_FAILED".to_string()),
                    timestamp: Utc::now(),
                }),
            ));
        }
    };

    let response = SubscribeResponse {
        subscription_id: subscription.id.clone(),
        stream_url: format!("/stream/{}", subscription.id),
        expires_at: subscription.expires_at,
    };

    info!(
        subscription_id = %subscription.id,
        client_id = ?req.client_id,
        expires_at = %subscription.expires_at,
        "Subscription created successfully"
    );

    Ok(Json(response))
}

/// Get subscription status
///
/// Returns information about an existing subscription, including whether
/// it's active, expired, or connected via WebSocket.
#[utoipa::path(
    get,
    path = "/subscribe/{subscription_id}",
    responses(
        (status = 200, description = "Subscription status", body = SubscriptionStatus),
        (status = 404, description = "Subscription not found", body = ErrorResponse),
    ),
    tag = "streaming"
)]
#[instrument(skip(state))]
pub async fn get_subscription_status(
    State(state): State<AppState>,
    Path(subscription_id): Path<String>,
) -> Result<Json<SubscriptionStatus>, (StatusCode, Json<ErrorResponse>)> {
    let status_info = match state.execution_manager.get_subscription_status(&subscription_id).await {
        Some(status) => status,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Subscription '{}' not found", subscription_id),
                    code: Some("SUBSCRIPTION_NOT_FOUND".to_string()),
                    timestamp: Utc::now(),
                }),
            ));
        }
    };

    // Convert internal type to API type
    let status = SubscriptionStatus {
        subscription_id: status_info.subscription_id,
        status: status_info.status,
        created_at: status_info.created_at,
        expires_at: status_info.expires_at,
        buffered_events: status_info.buffered_events,
        connected: status_info.connected,
        client_id: status_info.client_id,
    };

    Ok(Json(status))
}