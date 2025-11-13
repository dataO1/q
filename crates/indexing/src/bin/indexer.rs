use ai_agent_common::{config::SystemConfig, llm::EmbeddingClient};
use ai_agent_indexing::pipeline::IndexingCoordinator;
use ai_agent_indexing::watcher::FileWatcher;
use ai_agent_indexing::classifier::PathClassifier;
use ai_agent_storage::QdrantClient;
use anyhow::Result;
use tracing::{debug, error, info};
use clap::Parser;
use tracing_subscriber::EnvFilter;
use tracing::Level;


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
    let cli = Cli::parse();

    // Enhanced logging with backtrace support
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(Level::DEBUG.into())  // Default to DEBUG
                .add_directive("swiftide=trace".parse().unwrap())  // ← Swiftide TRACE
                .add_directive("ai_agent_indexing=trace".parse().unwrap())
                .add_directive("ai_agent_storage=trace".parse().unwrap())
        )
        .with_target(true)  // Show module paths
        .with_line_number(true)  // Show line numbers
        .with_thread_ids(true)
        .init();

    info!("🚀 Starting AI Agent File Indexer");
    info!("📄 Using config: {}", cli.config);

    // Load configuration
    let config = SystemConfig::load_config(&cli.config)?;
    debug!("Loaded config: {:#?}", config);  // ← Add debug

    let embedding_client = EmbeddingClient::new(&config.embedding.dense_model, config.embedding.vector_size)?;
    // Verify services
    info!("🔍 Verifying services...");
    if let Err(e) = verify_services(&config, &embedding_client).await {
        error!("Service verification failed: {:?}", e);
        error!("Backtrace: {:?}", std::backtrace::Backtrace::force_capture());
        return Err(e);
    }

    // Initialize coordinator with debug
    debug!("Creating IndexingCoordinator...");
    let coordinator = IndexingCoordinator::new(config.clone(), &embedding_client)
        .map_err(|e| {
            error!("Failed to create coordinator: {:?}", e);
            error!("Backtrace: {:?}", std::backtrace::Backtrace::force_capture());
            e
        })?;

    // Initial indexing
    info!("📚 Starting initial indexing...");
    coordinator.initial_index().await?;
    info!("✅ Initial indexing complete");


    // Event loop
    if config.indexing.watch_enabled {
        // Collect all paths to watch
        let mut watch_paths = Vec::new();
        watch_paths.extend(config.indexing.workspace_paths.clone());
        watch_paths.extend(config.indexing.personal_paths.clone());
        watch_paths.extend(config.indexing.system_paths.clone());
        let classifier = PathClassifier::new(&config.indexing);

        // Create watcher with paths
        let mut watcher = FileWatcher::new(watch_paths, config.indexing.filters.clone())?;
        info!("👁️  File watcher is watching: {:?}", watcher.get_watched_paths());
        info!("✨ Indexer running! Press Ctrl+C to stop.");
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
    Ok(())
}

/// Verify required services are running
async fn verify_services(config: &SystemConfig, embedder: &EmbeddingClient) -> Result<()> {
    info!("🔍 Verifying services...");
    // Check Qdrant
    let client = QdrantClient::new(&config.storage.qdrant_url, embedder)?;
    // let client = Qdrant::try_from_url(&config.storage.qdrant_url)?.build()?;
    match client.health_check().await{
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
