//! OpenTelemetry and Jaeger tracing setup

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::error::AgentNetworkResult;

pub fn init_tracing() -> AgentNetworkResult<()> {
    init_tracing_with_level("info")
}

pub fn init_tracing_with_level(level: &str) -> AgentNetworkResult<()> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(format!("agent_network={},tower_http=debug", level)))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    // TODO: Week 8 - Add OpenTelemetry Jaeger integration
    // - Configure Jaeger exporter
    // - Set up spans for all major operations
    // - Add trace context propagation

    tracing::info!("Tracing initialized with level: {}", level);
    Ok(())
}
