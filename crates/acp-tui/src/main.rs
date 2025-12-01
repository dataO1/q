//! ACP TUI Client - Terminal User Interface for Agent Communication Protocol
//!
//! This application provides a modern, interactive terminal interface for communicating
//! with ACP servers. Features include real-time orchestration visualization, live
//! status updates via WebSocket, and a React-like component architecture.

mod application;
mod client;
mod components;
mod config;
mod error;
mod message;
mod models;
mod services;
mod utils;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use config::Config;
use tracing::{info, warn};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_appender::non_blocking;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments first to get log configuration
    let matches = Command::new("acp-tui")
        .version("0.1.0")
        .about("Terminal User Interface for Agent Communication Protocol")
        .arg(
            Arg::new("server")
                .short('s')
                .long("server")
                .value_name("URL")
                .help("ACP server URL")
                .default_value("http://localhost:9999"),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path"),
        )
        .arg(
            Arg::new("log-level")
                .long("log-level")
                .value_name("LEVEL")
                .help("Log level (trace, debug, info, warn, error)")
                .default_value("info"),
        )
        .arg(
            Arg::new("log-file")
                .long("log-file")
                .value_name("FILE")
                .help("Log file path (default: ./acp-tui.log)")
                .default_value("./acp-tui.log"),
        )
        .get_matches();

    // Initialize tracing with parsed arguments
    let log_file = matches.get_one::<String>("log-file").unwrap();
    let log_level = matches.get_one::<String>("log-level").unwrap();
    init_tracing(log_file, log_level)?;

    // Load configuration
    let config = Config::load(
        matches.get_one::<String>("config"),
        matches.get_one::<String>("server").unwrap(),
        matches.get_one::<String>("log-level").unwrap(),
    )?;

    info!("Starting ACP TUI client");
    info!("Server: {}", config.server_url);
    
    // Test connectivity to ACP server
    test_connectivity(&config).await?;

    // Initialize and run the Elm-based application
    let mut app = application::Application::new(config).await?;
    let result = app.run().await;
    app.cleanup()?;
    result?;

    info!("ACP TUI client shutting down");
    Ok(())
}

fn init_tracing(log_file: &str, log_level: &str) -> Result<()> {
    // Create logs directory
    std::fs::create_dir_all("./logs")
        .context("Failed to create logs directory")?;
    
    // Generate timestamped log file name for this session
    let timestamp = chrono::Utc::now().format("%Y-%m-%d-%H%M%S");
    let log_file_path = format!("./logs/acp-tui-{}.log", timestamp);
    
    // Set up enhanced tracing with environment filter
    // Default to trace level for comprehensive debugging
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))
        .or_else(|_| EnvFilter::try_new("trace"))  // Default to trace for debugging
        .context("Failed to create tracing filter")?;

    // Create timestamped file appender (each run gets its own file)
    let file_appender = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&log_file_path)
        .context("Failed to create log file")?;
    
    // Use non-blocking writer to prevent I/O from affecting TUI performance
    let (non_blocking_appender, _guard) = non_blocking(file_appender);
    
    // Keep the guard alive for the duration of the program
    std::mem::forget(_guard);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_span_events(fmt::format::FmtSpan::FULL)  // Log span enter/exit
                .with_writer(non_blocking_appender),
        )
        .with(filter)
        .try_init()
        .context("Failed to initialize tracing")?;

    info!(log_file = %log_file_path, "Tracing initialized with comprehensive logging");
    info!("All events will be logged to {} for debugging", log_file_path);

    Ok(())
}

async fn test_connectivity(config: &Config) -> Result<()> {
    info!("Testing connectivity to ACP server...");
    
    match client::test_connection(&config.server_url).await {
        Ok(health) => {
            info!("✓ Connected to ACP server successfully");
            info!("  Server status: {}", health.status);
            if let Some(message) = &health.message {
                info!("  Server message: {}", message);
            }
        }
        Err(e) => {
            warn!("⚠ Could not connect to ACP server: {}", e);
            warn!("  Make sure the ACP server is running at: {}", config.server_url);
            warn!("  Starting in offline mode...");
        }
    }
    
    Ok(())
}