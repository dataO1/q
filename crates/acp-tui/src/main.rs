//! ACP TUI Client - Terminal User Interface for Agent Communication Protocol
//!
//! This application provides a modern, interactive terminal interface for communicating
//! with ACP servers. Features include real-time orchestration visualization, live
//! status updates via WebSocket, and a React-like component architecture.

// mod app;
mod client;
// mod components;
mod config;
// mod events;
mod models;
// mod ui;
// mod websocket;

use anyhow::{Context, Result};
// use app::App;
use clap::{Arg, Command};
use config::Config;
use std::env;
use tracing::{info, warn};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing()?;
    
    // Parse command line arguments
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
        .get_matches();

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

    // TODO: Initialize and run the application
    // let mut app = App::new(config).await?;
    // app.run().await?;
    
    println!("ACP TUI client initialized successfully!");
    println!("Server: {}", config.server_url);
    println!("This is a placeholder - the full TUI will be implemented next.");

    info!("ACP TUI client shutting down");
    Ok(())
}

fn init_tracing() -> Result<()> {
    // Set up tracing with environment filter
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .context("Failed to create tracing filter")?;

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true),
        )
        .with(filter)
        .try_init()
        .context("Failed to initialize tracing")?;

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