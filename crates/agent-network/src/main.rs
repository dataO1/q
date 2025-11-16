//! Agent-Network binary entry point
//!
//! Initializes the orchestrator, loads configuration, and starts the ACP server.
//! Supports both interactive and one-shot modes.

use ai_agent_common::{AgentNetworkConfig, SystemConfig};
use ai_agent_network::{
    tracing_setup, Orchestrator, VERSION,
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "agent-network")]
#[command(version = VERSION)]
#[command(author = "Agent Network Contributors")]
#[command(about = "Dynamic multi-agent orchestration framework")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.dev.toml")]
    config: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, env = "RUST_LOG")]
    log_level: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the orchestrator server
    Server {
        /// Host to bind to
        #[arg(long)]
        host: Option<String>,

        /// Port to bind to
        #[arg(long)]
        port: Option<u16>,
    },
    /// Execute a single query and exit
    Execute {
        /// The query to execute
        query: String,
    },
    /// Validate configuration
    ValidateConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let log_level = cli.log_level.as_deref().unwrap_or("info");
    tracing_setup::init_tracing_with_level(log_level)?;

    info!("Agent-Network v{} starting", VERSION);

    // Load configuration
    let config = SystemConfig::load_config(&cli.config).map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?.agent_network;

    info!("Configuration loaded successfully");
    info!("Available agents: {}", config.agents.len());
    for agent in &config.agents {
        info!("  - {} ({}): {}", agent.id, agent.agent_type, agent.model);
    }

    // Handle commands
    match cli.command {
        Some(Commands::ValidateConfig) => {
            println!("✓ Configuration is valid");
            println!("  Agents: {}", config.agents.len());
            println!("  Available types: {:?}", config.available_agent_types());
            Ok(())
        }
        Some(Commands::Execute { query }) => {
            execute_query(config, &query).await
        }
        Some(Commands::Server { host, port }) => {
            let mut config = config;
            if let Some(h) = host {
                config.acp.host = h;
            }
            if let Some(p) = port {
                config.acp.port = p;
            }
            start_server(config).await
        }
        None => {
            // Default to server mode
            start_server(config).await
        }
    }
}

/// Execute a single query and output results
async fn execute_query(config: AgentNetworkConfig, query: &str) -> Result<()> {
    info!("Executing query: {}", query);

    let orchestrator = Orchestrator::new(config).await?;

    match orchestrator.execute_query(query).await {
        Ok(result) => {
            println!("{}", result);
            Ok(())
        }
        Err(e) => {
            error!("Query execution failed: {}", e);
            Err(e.into())
        }
    }
}

/// Start the ACP server
async fn start_server(config: AgentNetworkConfig) -> Result<()> {
    info!("Starting ACP server on {}", config.acp);

    let orchestrator = Orchestrator::new(config).await?;

    // Start ACP server
    ai_agent_network::acp::start_server(orchestrator).await?;

    Ok(())
}
