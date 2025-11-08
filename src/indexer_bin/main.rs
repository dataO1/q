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
use swiftide::{
    indexing,
    indexing::loaders::FileLoader,
    indexing::transformers::{ChunkCode, ChunkMarkdown, ChunkText, Embed},
    integrations::ollama::Ollama,
};
use swiftide_indexing::Pipeline;
use swiftide_integrations::{qdrant::Qdrant as SwiftideQdrant, treesitter::SupportedLanguages};
use std::{fs::read_to_string, path::PathBuf};
use tracing::info;
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

    let mut ollama = Ollama::default();
    ollama.with_default_embed_model(&config.ollama.embedding_model);

    tracing::info!("Connected to Ollama with User {:?}", ollama.options().user);

    db::ensure_collection(
        &qdrant_client,
        &config.qdrant.collection_name,
        config.qdrant.vector_size,
    )
    .await?;

    tracing::info!("Connected to Qdrant at {}", config.qdrant.url);

    // for path in args.paths{
    //
    //     let watch_path = Path::new(path.as_path());
    //
    //     // 1. First, index all existing files
    //     info!("Scanning existing files in: {:?}", watch_path);
    //     for entry in WalkDir::new(watch_path)
    //         .follow_links(true)
    //         .into_iter()
    //         .filter_map(|e| e.ok())
    //     {
    //         let path = entry.path();
    //         if path.is_file() {
    //             if let Some(ext) = path.extension() {
    //                 if ext == "rs" || ext == "py" || ext == "js" || ext == "md" {
    //                     info!("Indexing existing file: {:?}", path);
    //                     if let Err(e) = index_file(path, &config, &qdrant_client).await {
    //                         error!("Failed to index {:?}: {}", path, e);
    //                     }
    //                 }
    //             }
    //         }
    //     }
    // }

    info!("Initial scan complete. Starting file watcher...");

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
                if let Err(e) = index_file(&path, &config, &ollama).await {
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

fn get_file_loader(path: &PathBuf, config: &Config)->Result<FileLoader>{

    let parent_dir = path.parent()
        .and_then(|p| p.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid path"))?;

    let extension = path.extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| anyhow::anyhow!("File has no extension"))?;

    let available_extensions: &Vec<String> = &config.chunking.supported_filetypes;

    let active_extensions: Vec<String> = available_extensions
        .iter() // Iterate over references to String (&String)
        .filter(|s| s.contains(extension)) // Use contains() which accepts &str
        .cloned() // Clone the &String references into new String objects
        .collect();

    // only load files with the right extension
    let file_loader = FileLoader::new(parent_dir)
        .with_extensions(&active_extensions);
    tracing::info!("Created FileLoader for: {:?}, with active extensions: {:?}", parent_dir,active_extensions);
    Ok(file_loader)
}

fn create_pipeline(path: PathBuf, config: &Config, ollama: &Ollama,) -> Result<Pipeline<String>, anyhow::Error>{

    let chunking_strategy = determine_chunk_strategy(&path, config);
    // For a single file, use the parent directory with a filter

    let path_clone = path.clone();
    // local file loader
    let file_loader = get_file_loader(&path, config)?;

    // filter out only the file that exactly matches the original path
    let pipeline = indexing::Pipeline::from_loader(file_loader)
        .filter(move |node| {
            if let Ok(n) = node.as_ref() {
                let keep = n.path == path_clone;
                if keep {
                    tracing::info!("Pipeline kept file: {:?}", path_clone);
                }
                keep
            } else {
                false
            }
        });


    // Build and run the indexing pipeline based on file type
    let chunked_pipeline = match chunking_strategy {
        ChunkStrategy::Code { language } => {
            tracing::info!("Indexing {} code from {:?}", language, path);
            pipeline
                .then_chunk(ChunkCode::try_for_language_and_chunk_size(
                    language,
                    config.chunking.chunk_size_range.0..config.chunking.chunk_size_range.1
                )?)
        }
        ChunkStrategy::Markdown => {
            tracing::info!("Indexing markdown from {:?}", path);
            pipeline
                .then_chunk(ChunkMarkdown::from_chunk_range(
                    config.chunking.chunk_size_range.0..config.chunking.chunk_size_range.1
                ))
        }
        ChunkStrategy::PlainText => {
            tracing::info!("Indexing plain text from {:?}", path);
            pipeline
                .then_chunk(ChunkText::from_chunk_range(
                    config.chunking.chunk_size_range.0..config.chunking.chunk_size_range.1
                ))
        }
    };


    let embedded_pipeline = chunked_pipeline.then_in_batch(
        Embed::new(ollama.clone())
            .with_batch_size(config.chunking.batch_size)
    );

    tracing::info!("Indexing file: {:?} with strategy: {}", path, chunking_strategy);
    return Ok(embedded_pipeline)
}

async fn index_file(
    path: &PathBuf,
    config: &Config,
    ollama: &Ollama,
) -> Result<()> {

    // Create Swiftide Qdrant storage
    let qdrant_storage = SwiftideQdrant::builder()
        .batch_size(config.chunking.batch_size)
        .vector_size(config.ollama.embedding_dimensions)
        .collection_name(&config.qdrant.collection_name)
        .build()?;

    let pipeline = create_pipeline(path.clone(),config, ollama)?;

    pipeline.then_store_with(qdrant_storage)
    .run()
    .await?;

    tracing::info!("Successfully indexed: {:?}", path);
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
