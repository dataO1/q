//! OpenTelemetry and Jaeger tracing setup

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::error::AgentResult;

pub fn init_tracing() -> AgentResult<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agent_network=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // TODO: Week 8 - Add OpenTelemetry Jaeger integration
    // - Configure Jaeger exporter
    // - Set up spans for all major operations
    // - Add trace context propagation

    tracing::info!("Tracing initialized");
    Ok(())
}
