use ai_agent_common::config::{SystemConfig};
use ai_agent_indexing::pipeline::IndexingCoordinator;
use ai_agent_indexing::watcher::FileWatcher;
use ai_agent_indexing::classifier::PathClassifier;
use anyhow::Result;
use tracing::{info, error};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "indexer")]
#[command(about = "AI Agent File Indexer", long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI args
    let cli = Cli::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap())
        )
        .init();

    info!("🚀 Starting AI Agent File Indexer");
    info!("📄 Using config: {}", cli.config);

    // Load configuration
    let config = SystemConfig::load_config(&cli.config)?;

    // Verify services
    verify_services(&config).await?;

    // Initialize components
    let coordinator = IndexingCoordinator::new(config.clone())?;
    let classifier = PathClassifier::new(&config.indexing);

    // Collect all paths to watch
    let mut watch_paths = Vec::new();
    watch_paths.extend(config.indexing.workspace_paths.clone());
    watch_paths.extend(config.indexing.personal_paths.clone());
    watch_paths.extend(config.indexing.system_paths.clone());

    // Create watcher with paths
    let mut watcher = FileWatcher::new(watch_paths, config.indexing.filters.clone())?;

    // Initial indexing
    info!("📚 Starting initial indexing...");
    coordinator.initial_index().await?;
    info!("✅ Initial indexing complete");

    // Start watching
    info!("👁️  File watcher is active");
    info!("✨ Indexer running! Press Ctrl+C to stop.");

    // Event loop
    loop {
        // Watch returns a single event (it's async)
        match watcher.watch().await {
            Ok(event) => {
                info!("📝 File event: {} - {}", event.event_type(), event.path().display());

                // Classify the file (this is also async now)
                match classifier.classify(&event.path()).await {
                    Ok(result) => {
                        info!("📂 Classified as: {:?} tier", result.tier);

                        // Handle the event
                        match coordinator.handle_file_event(
                            &event.path(),
                            &event.event_type(),
                            result.tier,
                        ).await {
                            Ok(_) => info!("✅ Indexed: {}", event.path().display()),
                            Err(e) => error!("❌ Failed to index {}: {}", event.path().display(), e),
                        }
                    }
                    Err(e) => {
                        error!("Failed to classify {}: {}", event.path().display(), e);
                    }
                }
            }
            Err(e) => {
                error!("Watcher error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

/// Verify required services are running
async fn verify_services(config: &SystemConfig) -> Result<()> {
    info!("🔍 Verifying services...");

    // Check Qdrant
    match reqwest::get(&format!("{}/", config.storage.qdrant_url)).await {
        Ok(_) => info!("✅ Qdrant: {}", config.storage.qdrant_url),
        Err(e) => {
            error!("❌ Qdrant not available at {}: {}", config.storage.qdrant_url, e);
            return Err(anyhow::anyhow!("Qdrant not running"));
        }
    }

    // Check Redis
    if let Some(redis_url) = &config.storage.redis_url {
        match redis::Client::open(redis_url.as_str()) {
            Ok(client) => {
                match client.get_connection() {
                    Ok(_) => info!("✅ Redis: {}", redis_url),
                    Err(e) => {
                        error!("❌ Redis connection failed: {}", e);
                        return Err(anyhow::anyhow!("Redis not running"));
                    }
                }
            }
            Err(e) => {
                error!("❌ Redis client error: {}", e);
                return Err(anyhow::anyhow!("Redis error"));
            }
        }
    }

    // Check Ollama
    match reqwest::get("http://localhost:11434/api/tags").await {
        Ok(_) => info!("✅ Ollama: http://localhost:11434"),
        Err(e) => {
            error!("❌ Ollama not available: {}", e);
            return Err(anyhow::anyhow!("Ollama not running"));
        }
    }

    info!("✅ All services verified");
    Ok(())
}
