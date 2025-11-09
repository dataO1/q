use ai_agent_common::SystemConfig;
use ai_agent_orchestrator::{OllamaModel, OrchestratorSystem};

#[tokio::main]
async fn main() -> anyhow::Result<()> {  // ← Changed from Result<()> to anyhow::Result<()>
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load full system configuration from file
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config.toml".to_string());

    let config = SystemConfig::load(&config_path)?;

    // Initialize orchestrator system with full config
    let mut orchestrator = OrchestratorSystem::new(&config).await?;

    println!("Orchestrator service started.");
    println!("Using database: {}", config.storage.postgres_url);

    // Run indefinitely or wait for shutdown signal
    tokio::signal::ctrl_c().await?;

    println!("Shutting down orchestrator...");
    Ok(())
}
