use tower_http::trace::TraceLayer;

pub fn get_tracing_layer() -> TraceLayer<()> {
    TraceLayer::new_for_http()
}
