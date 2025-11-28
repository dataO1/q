use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use tower_http::{classify::{ServerErrorsAsFailures, SharedClassifier}, trace::TraceLayer};
use tracing::info_span;

/// Get the default tracing layer for HTTP requests
pub fn get_tracing_layer() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
}

/// Custom logging middleware for requests
pub async fn logging_middleware(
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();
    
    let span = info_span!(
        "http_request",
        method = %method,
        uri = %uri,
        version = ?version,
    );
    
    // Execute the request within the span
    let response = span.in_scope(|| next.run(request));
    
    response.await
}
