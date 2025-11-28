//! Agent-Network binary entry point
//!
//! Initializes the orchestrator, loads configuration, and starts the ACP server.
//! Supports both interactive and one-shot modes.

use ai_agent_common::SystemConfig;
use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{error, info};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "agent-network")]
#[command(version = "0.1.0")]
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
    /// Execute a single query via ACP endpoint (client mode)
    Execute {
        /// The query to execute
        query: String,

        /// Current working directory / project path for context
        #[arg(long, default_value = ".")]
        cwd: String,

        /// ACP server URL
        #[arg(long, default_value = "http://localhost:8080")]
        server_url: String,
    },
    /// Validate configuration
    ValidateConfig,
}

#[derive(Debug, Serialize)]
struct ExecuteRequest {
    query: String,
    cwd: String,
}

#[derive(Debug, Deserialize)]
struct ExecuteResponse {
    result: String,
    success: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing with OpenTelemetry
    let log_level = cli.log_level.as_deref().unwrap_or("info");
    ai_agent_common::init_tracing_with_level(log_level)?;

    info!("ACP Server v0.1.0 starting");

    // Load configuration
    let config = SystemConfig::load_config(&cli.config).map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Configuration loaded successfully");
    info!("Available agents: {}", config.agent_network.agents.len());
    for agent in &config.agent_network.agents {
        info!("  - {} ({}): {}", agent.id, agent.agent_type, agent.model);
    }

    // Handle commands
    let result = match cli.command {
        Some(Commands::ValidateConfig) => {
            println!("✓ Configuration is valid");
            println!("  Agents: {}", config.agent_network.agents.len());
            println!("  Available types: {:?}", config.agent_network.available_agent_types());
            Ok(())
        }
        Some(Commands::Execute { query, cwd, server_url }) => {
            execute_via_acp(&query, &cwd, &server_url).await
        }
        Some(Commands::Server { host, port }) => {
            let mut config = config;
            if let Some(h) = host {
                config.agent_network.acp.host = h;
            }
            if let Some(p) = port {
                config.agent_network.acp.port = p;
            }
            start_server(config).await
        }
        None => {
            // Default to server mode
            start_server(config).await
        }
    };
    ai_agent_common::shutdown_tracer();
    result
}

/// Execute query via ACP HTTP endpoint (client mode)
async fn execute_via_acp(query: &str, cwd: &str, server_url: &str) -> Result<()> {
    info!("Executing query via ACP server: {}", server_url);

    let client = reqwest::Client::new();

    let request = ExecuteRequest {
        query: query.to_string(),
        cwd: cwd.to_string(),
    };

    let response = client
        .post(format!("{}/execute", server_url))
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        error!("Server returned error status: {}", response.status());
        return Err(anyhow::anyhow!("Server error: {}", response.status()));
    }

    let execute_response: ExecuteResponse = response.json().await?;

    if execute_response.success {
        println!("{}", execute_response.result);
        Ok(())
    } else {
        error!("Query execution failed: {}", execute_response.result);
        Err(anyhow::anyhow!("Query failed: {}", execute_response.result))
    }
}

/// Start the ACP server
async fn start_server(config: SystemConfig) -> Result<()> {
    info!("Starting ACP server on {}:{}", config.agent_network.acp.host, config.agent_network.acp.port);
    
    // Use the comprehensive ACP server with OpenAPI documentation
    let server = ai_agent_api::AcpServer::new(config).await?;
    server.run().await?;

    Ok(())
}
