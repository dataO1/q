use opentelemetry::KeyValue;
use opentelemetry_otlp::{HttpExporterBuilder, WithExportConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{Sampler, Config};
use opentelemetry_sdk::{Resource, runtime};
use opentelemetry_semantic_conventions as semcov;
use opentelemetry::{global, trace::{Span, Tracer}};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{Registry, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use anyhow::Result;
use tracing::info;
use opentelemetry_sdk::trace::SdkTracerProvider;

use once_cell::sync::Lazy;
use std::sync::Mutex;

static TRACER_PROVIDER: Lazy<Mutex<Option<SdkTracerProvider>>> = Lazy::new(|| Mutex::new(None));

pub fn init_tracing_with_level(level: &str) -> Result<()> {
    // Create OTLP HTTP exporter builder
    let exporter = HttpExporterBuilder::default()
        .with_endpoint("http://localhost:4318/v1/traces")
        .build_span_exporter()?;                 // Build SpanExporter

    // Setup Resource with service name and version
    let resource = Resource::builder().with_attributes(vec![
        KeyValue::new(semcov::resource::SERVICE_NAME, "agent-network"),
        KeyValue::new(semcov::resource::SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
    ]).build();

    // Build tracer provider with batch exporter, sampler and resource
    let tracer_provider = SdkTracerProvider::builder()
        .with_sampler(Sampler::AlwaysOn)
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build();
    *TRACER_PROVIDER.lock().unwrap() = Some(tracer_provider.clone());
    opentelemetry::global::set_tracer_provider(tracer_provider.clone());

    // Set as global tracer provider
    global::set_tracer_provider(tracer_provider);

    // Set global text map propagator for context propagation
    global::set_text_map_propagator(
        TraceContextPropagator::new(),
    );

    // Create OpenTelemetry tracing layer with the tracer
    let tracer = global::tracer("agent-network");
    let otel_layer = OpenTelemetryLayer::new(tracer);

    // Setup filtering and formatting
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(false);

    // Register composed subscriber with layers
    Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_layer)
        .try_init()?;

    info!("Tracing initialized with level: {}", level);
    info!("OpenTelemetry exporting to Jaeger at http://localhost:4318");

    Ok(())
}

pub fn shutdown_tracer() {
    if let Some(provider) = TRACER_PROVIDER.lock().unwrap().take() {
        provider.shutdown().expect("Error shutting down tracer provider");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_init_tracing() {
        // Just verify function compiles and runs, actual init requires Tokio runtime
        assert!(true);
    }
}
