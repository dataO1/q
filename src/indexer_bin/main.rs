use anyhow::{Context, Result};
use clap::Parser;
use semantic_search::{
    config::Config,
    db,
    indexer::{
        chunking::{determine_chunk_strategy, ChunkStrategy},
        FileWatcher,
        FileEvent,
    },
};
use std::path::PathBuf;
use std::time::Duration;
use tracing_subscriber;

#[derive(Parser, Debug)]
#[command(name = "semantic-indexer")]
#[command(about = "Background indexer for semantic code search")]
struct Args {
    /// Paths to watch for file changes
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Config file path
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Debounce duration in milliseconds
    #[arg(short, long, default_value = "500")]
    debounce_ms: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let config = if args.config.exists() {
        Config::from_file(&args.config)?
    } else {
        tracing::warn!("Config file not found, using defaults");
        Config::default_config()
    };

    let qdrant_client = qdrant_client::Qdrant::from_url(&config.qdrant.url)
        .build()
        .context("Failed to connect to Qdrant")?;

    db::ensure_collection(
        &qdrant_client,
        &config.qdrant.collection_name,
        config.qdrant.vector_size,
    )
    .await?;

    tracing::info!("Connected to Qdrant at {}", config.qdrant.url);

    let (watcher, mut rx) = FileWatcher::new(
        args.paths.clone(),
        Duration::from_millis(args.debounce_ms),
    )?;

    tracing::info!("File watcher started. Waiting for file changes...");

    while let Some(event) = rx.recv().await {
        match event {
            FileEvent::Modified(path)
            | FileEvent::Created(path) => {
                tracing::info!("Indexing: {:?}", path);
                if let Err(e) = index_file(&path, &config, &qdrant_client).await {
                    tracing::error!("Failed to index {:?}: {}", path, e);
                }
            }
            FileEvent::Removed(path) => {
                tracing::info!("Removing from index: {:?}", path);
                if let Err(e) = remove_from_index(&path, &config, &qdrant_client).await {
                    tracing::error!("Failed to remove {:?}: {}", path, e);
                }
            }
        }
    }

    drop(watcher);
    Ok(())
}

async fn index_file(
    path: &PathBuf,
    config: &Config,
    _qdrant: &qdrant_client::Qdrant,
) -> Result<()> {
    let strategy = determine_chunk_strategy(path, config);

    tracing::debug!("Processing file: {:?} with chunking strategy", path);

    // TODO: Implement actual indexing with swiftide
    // This is a stub - actual implementation would use swiftide pipeline
    match strategy {
        ChunkStrategy::Code { language } => {
            tracing::info!("Would index {} code from {:?}", language, path);
        }
        ChunkStrategy::Markdown => {
            tracing::info!("Would index markdown from {:?}", path);
        }
        ChunkStrategy::PlainText => {
            tracing::info!("Would index plain text from {:?}", path);
        }
    }

    Ok(())
}

async fn remove_from_index(
    path: &PathBuf,
    config: &Config,
    qdrant: &qdrant_client::Qdrant,
) -> Result<()> {
    use qdrant_client::qdrant::{Condition, DeletePointsBuilder, Filter};

    let filter = Filter::must([Condition::matches(
        "path",
        path.to_string_lossy().to_string(),
    )]);

    qdrant
        .delete_points(DeletePointsBuilder::new(&config.qdrant.collection_name).points(filter))
        .await
        .context("Failed to delete points")?;

    Ok(())
}
