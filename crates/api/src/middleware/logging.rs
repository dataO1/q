use tower_http::{classify::{ServerErrorsAsFailures, SharedClassifier}, trace::TraceLayer};

pub fn get_tracing_layer() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
}
