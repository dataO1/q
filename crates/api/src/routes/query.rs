use axum::{
    http::StatusCode,
    Json,
    extract::{State, Request},
    response::Result,
    body::Bytes
};
use ai_agent_common::ConversationId;
use tracing::{info, error, warn, instrument};
use chrono::Utc;
use crate::{
    types::{QueryRequest, QueryResponse, ErrorResponse},
    server::AppState,
};

/// Execute a query asynchronously
///
/// Starts background execution of a user query and returns immediately with a conversation_id.
/// Real-time progress updates are available via WebSocket streaming at the returned stream_url.
///
/// ## Behavior
///
/// - **Immediate Response**: Returns conversation_id without waiting for completion
/// - **Background Execution**: Query processing happens asynchronously
/// - **Streaming Updates**: Connect to WebSocket for real-time progress
/// - **Conversation Context**: Reuse conversation_id for related queries
///
/// ## Project Scope
///
/// The `project_scope` field must be populated by the client with:
/// - Project root directory path
/// - Detected programming languages
/// - Key files and their purposes
/// - Active development areas
///
/// This context allows agents to understand the codebase structure and provide
/// relevant assistance with appropriate tools.
///
/// ## WebSocket Events
///
/// After calling this endpoint, connect to the WebSocket stream to receive:
///
/// 1. `ExecutionStarted` - Processing has begun
/// 2. `AgentStarted` - An agent has been assigned
/// 3. `WorkflowStepStarted/Completed` - Step-by-step progress
/// 4. `AgentThinking` - Intermediate thoughts (optional)
/// 5. `AgentCompleted` - Agent finished with results
/// 6. `ExecutionCompleted` - Final results available
///
/// ## Error Handling
///
/// - API errors return `ErrorResponse` immediately
/// - Execution errors are streamed as `ExecutionFailed` events
/// - WebSocket disconnections don't affect background processing
///
/// ## Example Usage
///
/// ```bash
/// # Start execution
/// curl -X POST /query \
///   -H "Content-Type: application/json" \
///   -d '{"query": "Analyze the auth module", "project_scope": {...}}'
///
/// # Connect to stream
/// wscat -c "ws://localhost:3000/stream/{conversation_id}"
/// ```
/// Extract JSON body and parse with detailed error reporting
#[instrument(skip(state, request))]
async fn extract_and_validate_json(
    state: &AppState,
    request: Request,
) -> Result<QueryRequest, (StatusCode, Json<ErrorResponse>)> {
    // Extract body
    let body = match axum::body::to_bytes(request.into_body(), 10_000_000).await {
        Ok(body) => body,
        Err(e) => {
            error!(error = %e, "Failed to read request body");
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Failed to read request body: {}", e),
                    code: Some("BODY_READ_ERROR".to_string()),
                    timestamp: Utc::now(),
                })
            ));
        }
    };

    // Log the raw request body for debugging
    let body_str = String::from_utf8_lossy(&body);
    info!(
        body_size = body.len(),
        body_preview = %body_str.chars().take(300).collect::<String>(),
        "Processing request body"
    );

    // Try to parse as generic JSON first to provide detailed field-level errors
    let json_value = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) => {
            info!(
                has_query = value.get("query").is_some(),
                has_project_scope = value.get("project_scope").is_some(),
                has_subscription_id = value.get("subscription_id").is_some(),
                "JSON structure parsed successfully"
            );
            value
        }
        Err(e) => {
            error!(
                error = %e,
                line = e.line(),
                column = e.column(),
                body_preview = %body_str.chars().take(500).collect::<String>(),
                "JSON parsing failed"
            );
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid JSON at line {}, column {}: {}", e.line(), e.column(), e),
                    code: Some("JSON_PARSE_ERROR".to_string()),
                    timestamp: Utc::now(),
                })
            ));
        }
    };

    // Validate required fields with specific error messages
    if json_value.get("query").is_none() {
        error!("Missing required field: 'query'");
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "Missing required field 'query'. The request must include a query string.".to_string(),
                code: Some("MISSING_FIELD_QUERY".to_string()),
                timestamp: Utc::now(),
            })
        ));
    }

    if json_value.get("project_scope").is_none() {
        error!("Missing required field: 'project_scope'");
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "Missing required field 'project_scope'. The request must include project context.".to_string(),
                code: Some("MISSING_FIELD_PROJECT_SCOPE".to_string()),
                timestamp: Utc::now(),
            })
        ));
    }

    if json_value.get("subscription_id").is_none() {
        error!("Missing required field: 'subscription_id'");
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "Missing required field 'subscription_id'. Please create a subscription with POST /subscribe first.".to_string(),
                code: Some("MISSING_FIELD_SUBSCRIPTION_ID".to_string()),
                timestamp: Utc::now(),
            })
        ));
    }

    // Validate project_scope subfields
    if let Some(project_scope) = json_value.get("project_scope") {
        if !project_scope.is_object() {
            error!("Invalid project_scope format: not an object");
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: "Field 'project_scope' must be an object with required fields.".to_string(),
                    code: Some("INVALID_FIELD_PROJECT_SCOPE_TYPE".to_string()),
                    timestamp: Utc::now(),
                })
            ));
        }

        if project_scope.get("root").is_none() {
            error!("Missing required field: 'project_scope.root'");
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: "Missing required field 'project_scope.root'. Please provide the project root directory path.".to_string(),
                    code: Some("MISSING_FIELD_PROJECT_SCOPE_ROOT".to_string()),
                    timestamp: Utc::now(),
                })
            ));
        }

        if project_scope.get("language_distribution").is_none() {
            error!("Missing required field: 'project_scope.language_distribution'");
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: "Missing required field 'project_scope.language_distribution'. Please provide programming language distribution as an object with language names as keys and percentages as values.".to_string(),
                    code: Some("MISSING_FIELD_LANGUAGE_DISTRIBUTION".to_string()),
                    timestamp: Utc::now(),
                })
            ));
        }

        // Validate language_distribution is an object
        if let Some(lang_dist) = project_scope.get("language_distribution") {
            if !lang_dist.is_object() {
                error!("Invalid language_distribution format: expected object, got {}",
                       if lang_dist.is_array() { "array" } else { "primitive" });
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "Field 'project_scope.language_distribution' must be an object with language names as keys and percentages as values, like {\"rust\": 0.8, \"typescript\": 0.2}.".to_string(),
                        code: Some("INVALID_FIELD_LANGUAGE_DISTRIBUTION_TYPE".to_string()),
                        timestamp: Utc::now(),
                    })
                ));
            }

            // Validate object contains valid key-value pairs
            if let Some(obj) = lang_dist.as_object() {
                for (key, value) in obj.iter() {
                    if !value.is_number() {
                        error!("Invalid language_distribution value for key '{}': expected number (percentage)", key);
                        return Err((
                            StatusCode::UNPROCESSABLE_ENTITY,
                            Json(ErrorResponse {
                                error: format!("Value for language '{}' in 'language_distribution' must be a number (percentage), not a {}.",
                                              key, if value.is_string() { "string" } else { "non-number value" }),
                                code: Some("INVALID_LANGUAGE_DISTRIBUTION_PERCENTAGE".to_string()),
                                timestamp: Utc::now(),
                            })
                        ));
                    }
                }
            }
        }
    }

    // Now attempt to deserialize the complete structure with detailed error reporting
    match serde_json::from_value::<QueryRequest>(json_value.clone()) {
        Ok(req) => {
            info!(
                query_length = req.query.len(),
                project_root = %req.project_scope.root,
                language_count = req.project_scope.language_distribution.len(),
                subscription_id = %req.subscription_id,
                "Request successfully parsed and validated"
            );
            Ok(req)
        }
        Err(e) => {
            error!(
                error = %e,
                json_preview = %serde_json::to_string(&json_value).unwrap_or_else(|_| "failed to serialize".to_string()).chars().take(500).collect::<String>(),
                "Failed to deserialize QueryRequest structure"
            );

            // Try to provide more specific error information
            let error_msg = if e.to_string().contains("language_distribution") {
                "Invalid 'language_distribution' format. Expected an object with language names as keys and percentages as values like {\"rust\": 0.8, \"typescript\": 0.2}.".to_string()
            } else if e.to_string().contains("missing field") {
                format!("Missing required field in request structure: {}", e)
            } else {
                format!("Invalid request structure: {}", e)
            };

            Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: error_msg,
                    code: Some("INVALID_REQUEST_STRUCTURE".to_string()),
                    timestamp: Utc::now(),
                })
            ))
        }
    }
}

/// Validate parsed query request with detailed logging
#[instrument(skip(_state, req))]
async fn validate_parsed_query_request(
    _state: &AppState,
    req: QueryRequest,
) -> Result<QueryRequest, (StatusCode, Json<ErrorResponse>)> {
    info!(
        query_length = req.query.len(),
        query_preview = %req.query.chars().take(100).collect::<String>(),
        project_root = %req.project_scope.root,
        language_count = req.project_scope.language_distribution.len(),
        subscription_id = %req.subscription_id,
        "Validating parsed query request"
    );

    // Basic validation - Axum already handled JSON parsing
    if req.query.trim().is_empty() {
        warn!("Empty query provided");
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "Query cannot be empty. Please provide a query string.".to_string(),
                code: Some("EMPTY_QUERY".to_string()),
                timestamp: Utc::now(),
            })
        ));
    }

    if req.project_scope.root.trim().is_empty() {
        warn!("Empty project root provided");
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "Project root cannot be empty. Please provide the project root directory path.".to_string(),
                code: Some("EMPTY_PROJECT_ROOT".to_string()),
                timestamp: Utc::now(),
            })
        ));
    }

    if req.project_scope.language_distribution.is_empty() {
        warn!("Empty language distribution provided");
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "Language distribution cannot be empty. Please provide programming language distribution.".to_string(),
                code: Some("EMPTY_LANGUAGE_DISTRIBUTION".to_string()),
                timestamp: Utc::now(),
            })
        ));
    }

    info!(
        query_length = req.query.len(),
        project_root = %req.project_scope.root,
        language_count = req.project_scope.language_distribution.len(),
        "Query request validated successfully"
    );
    Ok(req)
}

#[utoipa::path(
    post,
    path = "/query",
    tag = "execution",
    request_body = QueryRequest,
    responses(
        (status = 200, description = "Execution started successfully", body = QueryResponse),
        (status = 500, description = "Failed to start execution", body = ErrorResponse)
    )
)]
#[instrument(skip(state, request))]
pub async fn query_task(
    State(state): State<AppState>,
    request: Request,
) -> Result<Json<QueryResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Extract and parse JSON with detailed error logging
    let req = extract_and_validate_json(&state, request).await?;
    info!(
        query_length = %req.query.len(),
        query_preview = %req.query.chars().take(100).collect::<String>(),
        project_root = %req.project_scope.root,
        language_count = %req.project_scope.language_distribution.len(),
        subscription_id = %req.subscription_id,
        "Starting query execution"
    );

    // Use project scope provided by client
    let project_scope = req.project_scope.clone();
    let project_root = project_scope.root.clone(); // Clone for later use in error logging

    // Execute query through execution manager with subscription_id (returns immediately, runs async)
    let result = state.execution_manager.execute_query(
        &req.query,
        project_scope,
        &req.subscription_id
    ).await;

    match result {
        Ok(()) => {
            info!(
                subscription_id = %req.subscription_id,
                query_length = %req.query.len(),
                "Query execution started successfully"
            );
        }
        Err(e) => {
            error!(
                error = %e,
                query_preview = %req.query.chars().take(100).collect::<String>(),
                project_root = %project_root,
                subscription_id = %req.subscription_id,
                "Failed to start query execution"
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to start execution: {}", e),
                    code: Some("EXECUTION_START_FAILED".to_string()),
                    timestamp: Utc::now(),
                })
            ));
        }
    }

    let response = QueryResponse {
        subscription_id: req.subscription_id.clone(),
        stream_url: format!("/stream/{}", req.subscription_id),
        status: "started".to_string(),
    };

    info!("Execution started successfully for subscription {}", req.subscription_id);
    Ok(Json(response))
}
