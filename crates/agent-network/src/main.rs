//! Agent-Network binary entry point

use ai_agent_common::{AgentNetworkConfig, SystemConfig};
use ai_agent_network::{acp::start_server, tracing_setup::init_tracing, Orchestrator};
use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {

    let _ = env_logger::builder().is_test(true).try_init();
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config.dev.toml".to_string());
    let config = SystemConfig::from_file(&config_path).unwrap().agent_network;
    // Initialize tracing
    init_tracing()?;

    info!("Starting agent-network service");

    // Initialize orchestrator
    let orchestrator = Orchestrator::new(config).await?;

    // Start ACP server
    start_server(orchestrator).await?;

    Ok(())
}
