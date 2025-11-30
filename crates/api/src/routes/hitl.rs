//! HITL (Human-in-the-Loop) API routes
//!
//! Provides endpoints for managing human approval workflows in agent execution.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

use crate::types::{
    ErrorResponse, HitlApprovalRequest, HitlDecisionRequest, HitlDecisionResponse,
    HitlPendingResponse, HitlRequestDetails, HitlMetadata, TaskContext,
};
use crate::server::AppState;

/// In-memory storage for HITL requests (TODO: replace with proper persistence)
/// This will be integrated into the AppState later
static HITL_STORAGE: once_cell::sync::Lazy<Arc<RwLock<HashMap<String, HitlApprovalRequest>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

/// Get HITL storage reference
fn get_hitl_storage() -> &'static Arc<RwLock<HashMap<String, HitlApprovalRequest>>> {
    &HITL_STORAGE
}

/// Get all pending HITL requests
///
/// Returns a list of approval requests that are waiting for human decision.
/// Requests are returned in chronological order (oldest first) to help users
/// understand the context of why HITL was triggered.
#[utoipa::path(
    get,
    path = "/hitl/pending",
    responses(
        (status = 200, description = "List of pending HITL requests", body = HitlPendingResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "HITL"
)]
pub async fn get_pending_requests(
    State(_state): State<AppState>,
) -> Result<Json<HitlPendingResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!("Getting pending HITL requests");
    
    let storage = get_hitl_storage().read().map_err(|e| {
        warn!("Failed to acquire read lock on HITL storage: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to access HITL storage".to_string(),
                code: Some("STORAGE_LOCK_ERROR".to_string()),
                timestamp: Utc::now(),
            }),
        )
    })?;
    
    // Get requests in chronological order (oldest first)
    let mut requests: Vec<HitlApprovalRequest> = storage.values().cloned().collect();
    requests.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    
    let count = requests.len();
    info!("Found {} pending HITL requests", count);
    
    Ok(Json(HitlPendingResponse { requests, count }))
}

/// Submit a decision for a HITL request
///
/// Processes a human decision (approve, reject, or modify) for a pending
/// approval request. Once processed, the request is removed from the queue
/// and the decision is applied to the agent workflow.
#[utoipa::path(
    post,
    path = "/hitl/decide",
    request_body = HitlDecisionRequest,
    responses(
        (status = 200, description = "Decision processed successfully", body = HitlDecisionResponse),
        (status = 404, description = "Request not found", body = ErrorResponse),
        (status = 400, description = "Invalid decision", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "HITL"
)]
pub async fn submit_decision(
    State(_state): State<AppState>,
    Json(decision): Json<HitlDecisionRequest>,
) -> Result<Json<HitlDecisionResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Processing HITL decision for request {}: {:?}", decision.request_id, decision.decision);
    
    // Validate the decision
    if matches!(decision.decision, crate::types::HitlDecision::Modify) && decision.modified_content.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Modified content required when decision is 'Modify'".to_string(),
                code: Some("MISSING_MODIFIED_CONTENT".to_string()),
                timestamp: Utc::now(),
            }),
        ));
    }
    
    // Remove the request from storage
    let mut storage = get_hitl_storage().write().map_err(|e| {
        warn!("Failed to acquire write lock on HITL storage: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to access HITL storage".to_string(),
                code: Some("STORAGE_LOCK_ERROR".to_string()),
                timestamp: Utc::now(),
            }),
        )
    })?;
    
    let request = storage.remove(&decision.request_id).ok_or_else(|| {
        warn!("HITL request not found: {}", decision.request_id);
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("HITL request '{}' not found", decision.request_id),
                code: Some("REQUEST_NOT_FOUND".to_string()),
                timestamp: Utc::now(),
            }),
        )
    })?;
    
    // TODO: Apply the decision to the agent workflow
    // This would involve notifying the waiting agent about the decision
    
    info!("Successfully processed HITL decision for request {} (task: {})", 
          request.request_id, request.task_id);
    
    Ok(Json(HitlDecisionResponse {
        request_id: decision.request_id,
        decision: decision.decision,
        processed_at: Utc::now(),
        message: "Decision processed successfully".to_string(),
    }))
}

/// Get detailed information about a specific HITL request
///
/// Returns complete details about a HITL request including metadata
/// and context information to help with decision making.
#[utoipa::path(
    get,
    path = "/hitl/{request_id}/details",
    params(
        ("request_id" = String, Path, description = "HITL request ID")
    ),
    responses(
        (status = 200, description = "HITL request details", body = HitlRequestDetails),
        (status = 404, description = "Request not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "HITL"
)]
pub async fn get_request_details(
    State(_state): State<AppState>,
    Path(request_id): Path<String>,
) -> Result<Json<HitlRequestDetails>, (StatusCode, Json<ErrorResponse>)> {
    debug!("Getting details for HITL request: {}", request_id);
    
    let storage = get_hitl_storage().read().map_err(|e| {
        warn!("Failed to acquire read lock on HITL storage: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to access HITL storage".to_string(),
                code: Some("STORAGE_LOCK_ERROR".to_string()),
                timestamp: Utc::now(),
            }),
        )
    })?;
    
    let request = storage.get(&request_id).cloned().ok_or_else(|| {
        warn!("HITL request not found: {}", request_id);
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("HITL request '{}' not found", request_id),
                code: Some("REQUEST_NOT_FOUND".to_string()),
                timestamp: Utc::now(),
            }),
        )
    })?;
    
    let pending_duration = Utc::now()
        .signed_duration_since(request.timestamp)
        .num_milliseconds() as u64;
    
    let metadata = HitlMetadata {
        execution_id: "unknown".to_string(), // TODO: get from execution context
        status: "pending".to_string(),
        pending_duration_ms: pending_duration,
        task_context: TaskContext {
            description: request.proposed_action.clone(),
            wave_index: 0, // TODO: get from execution context
            dependencies: vec![], // TODO: get from execution context
        },
    };
    
    info!("Retrieved details for HITL request: {}", request_id);
    
    Ok(Json(HitlRequestDetails {
        request,
        metadata,
    }))
}

/// Add a new HITL request (internal function for agent use)
///
/// This function is called internally by agents when they need approval.
/// It's not exposed as a public API endpoint.
pub async fn add_hitl_request(
    request: HitlApprovalRequest,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Adding new HITL request: {} for task: {}", request.request_id, request.task_id);
    
    let mut storage = get_hitl_storage().write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
    
    storage.insert(request.request_id.clone(), request);
    
    info!("HITL request added to queue");
    Ok(())
}

/// Get the current number of pending requests (for monitoring)
pub async fn get_pending_count() -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let storage = get_hitl_storage().read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
    
    Ok(storage.len())
}