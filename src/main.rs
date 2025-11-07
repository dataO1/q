use anyhow::{Context, Result};
use clap::Parser;
use semantic_search::{config::Config, db, indexer::FileWatcher};
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

    // Load config
    let config = if args.config.exists() {
        Config::from_file(&args.config)?
    } else {
        tracing::warn!("Config file not found, using defaults");
        Config::default_config()
    };

    // Initialize Qdrant
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

    // Start file watcher
    let (watcher, mut rx) = FileWatcher::new(
        args.paths.clone(),
        Duration::from_millis(args.debounce_ms),
    )?;

    tracing::info!("File watcher started. Waiting for file changes...");

    // Process events
    while let Some(event) = rx.recv().await {
        match event {
            semantic_search::indexer::watcher::FileEvent::Modified(path)
            | semantic_search::indexer::watcher::FileEvent::Created(path) => {
                tracing::info!("Indexing: {:?}", path);
                if let Err(e) = index_file(&path, &config, &qdrant_client).await {
                    tracing::error!("Failed to index {:?}: {}", path, e);
                }
            }
            semantic_search::indexer::watcher::FileEvent::Removed(path) => {
                tracing::info!("Removing from index: {:?}", path);
                if let Err(e) = remove_from_index(&path, &config, &qdrant_client).await {
                    tracing::error!("Failed to remove {:?}: {}", path, e);
                }
            }
        }
    }

    // Keep watcher alive
    drop(watcher);
    Ok(())
}

async fn index_file(
    path: &PathBuf,
    config: &Config,
    qdrant: &qdrant_client::Qdrant,
) -> Result<()> {
    use semantic_search::indexer::chunking::{determine_chunk_strategy, ChunkStrategy};
    use swiftide::integrations::{ollama::Ollama, qdrant::Qdrant as SwiftideQdrant};
    use swiftide::indexing::{self, loaders::FileLoader, transformers::{ChunkCode, ChunkMarkdown, Embed}};

    let strategy = determine_chunk_strategy(path, config);

    let ollama_client = Ollama::builder()
        .default_embed_model(&config.ollama.embedding_model)
        .build()?;

    let qdrant_storage = SwiftideQdrant::builder()
        .batch_size(10)
        .collection_name(&config.qdrant.collection_name)
        .vector_size(config.qdrant.vector_size as usize)
        .build()?;

    match strategy {
        ChunkStrategy::Code { language } => {
            indexing::Pipeline::from_loader(
                FileLoader::new(path.parent().unwrap())
                    .with_extensions(&[path.extension().unwrap().to_str().unwrap()])
            )
            .filter_cached(|node| {
                node.path == path.to_string_lossy().to_string()
            })
            .then_chunk(ChunkCode::try_for_language_and_chunk_size(
                &language,
                config.chunking.chunk_size_range.0..config.chunking.chunk_size_range.1,
            )?)
            .then_in_batch(10, Embed::new(ollama_client))
            .then_store_with(qdrant_storage)
            .run()
            .await?;
        }
        ChunkStrategy::Markdown => {
            indexing::Pipeline::from_loader(
                FileLoader::new(path.parent().unwrap())
                    .with_extensions(&["md"])
            )
            .filter_cached(|node| {
                node.path == path.to_string_lossy().to_string()
            })
            .then_chunk(ChunkMarkdown::from_chunk_range(
                config.chunking.chunk_size_range.0..config.chunking.chunk_size_range.1
            ))
            .then_in_batch(10, Embed::new(ollama_client))
            .then_store_with(qdrant_storage)
            .run()
            .await?;
        }
        ChunkStrategy::PlainText => {
            // Use text splitter for unknown file types
            use swiftide::indexing::transformers::ChunkText;

            indexing::Pipeline::from_loader(
                FileLoader::new(path.parent().unwrap())
            )
            .filter_cached(|node| {
                node.path == path.to_string_lossy().to_string()
            })
            .then_chunk(ChunkText::from_chunk_range(
                config.chunking.chunk_size_range.0..config.chunking.chunk_size_range.1
            ))
            .then_in_batch(10, Embed::new(ollama_client))
            .then_store_with(qdrant_storage)
            .run()
            .await?;
        }
    }

    Ok(())
}

async fn remove_from_index(
    path: &PathBuf,
    config: &Config,
    qdrant: &qdrant_client::Qdrant,
) -> Result<()> {
    use qdrant_client::qdrant::{Condition, Filter, DeletePointsBuilder};

    // Delete points with matching file path in payload
    let filter = Filter::must([
        Condition::matches("path", path.to_string_lossy().to_string())
    ]);

    qdrant
        .delete_points(
            DeletePointsBuilder::new(&config.qdrant.collection_name)
                .points(filter)
        )
        .await
        .context("Failed to delete points")?;

    Ok(())
}
