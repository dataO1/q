//! Agent-Network binary entry point

use agent_network::{AgentNetworkConfig, Orchestrator};
use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    agent_network::tracing_setup::init_tracing()?;

    info!("Starting agent-network service");

    // Load configuration
    let config = AgentNetworkConfig::load("config.toml")?;

    // Initialize orchestrator
    let orchestrator = Orchestrator::new(config).await?;

    // Start ACP server
    agent_network::acp::start_server(orchestrator).await?;

    Ok(())
}
