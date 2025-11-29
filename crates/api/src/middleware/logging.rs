use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
    body::Body,
};
use tower_http::{classify::{ServerErrorsAsFailures, SharedClassifier}, trace::TraceLayer};
use tracing::{info_span, info, error, instrument};
use std::time::Instant;
use uuid::Uuid;

/// Get the default tracing layer for HTTP requests
pub fn get_tracing_layer() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
}

/// Custom logging middleware for requests with detailed request/response tracking
#[instrument(skip(request, next))]
pub async fn logging_middleware(
    mut request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4().to_string();
    
    // Extract request details
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();
    let headers = request.headers().clone();
    
    // Extract user agent and client info
    let user_agent = headers.get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    
    let content_type = headers.get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
        
    let content_length = headers.get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    
    // Add request ID to request headers for downstream use
    request.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap()
    );
    
    info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        version = ?version,
        user_agent = %user_agent,
        content_type = %content_type,
        content_length = %content_length,
        "Incoming HTTP request"
    );
    
    let span = info_span!(
        "http_request",
        request_id = %request_id,
        method = %method,
        uri = %uri,
        version = ?version,
    );
    
    // Execute the request within the span
    let response = span.in_scope(|| next.run(request)).await;
    
    // Log response details
    let duration = start_time.elapsed();
    let status = response.status();
    let response_headers = response.headers().clone();
    
    let response_content_length = response_headers.get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    
    if status.is_success() {
        info!(
            request_id = %request_id,
            status = %status,
            duration_ms = %duration.as_millis(),
            response_size = %response_content_length,
            "HTTP request completed successfully"
        );
    } else if status.is_client_error() {
        error!(
            request_id = %request_id,
            status = %status,
            duration_ms = %duration.as_millis(),
            "HTTP request failed with client error"
        );
    } else if status.is_server_error() {
        error!(
            request_id = %request_id,
            status = %status,
            duration_ms = %duration.as_millis(),
            "HTTP request failed with server error"
        );
    } else {
        info!(
            request_id = %request_id,
            status = %status,
            duration_ms = %duration.as_millis(),
            "HTTP request completed"
        );
    }
    
    response
}
