use ai_agent_common::*;
use ai_agent_storage::QdrantClient;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

// Correct Swiftide 0.32 imports
use swiftide::indexing::{
    EmbeddedField,
    loaders::FileLoader,
    transformers::{self, ChunkCode, ChunkMarkdown, MetadataQACode, Embed},
    Pipeline,
};
use swiftide::integrations::{
    ollama::Ollama,
    fastembed::FastEmbed,
    qdrant::Qdrant as SwiftideQdrant,
    redis::Redis,
};

use tracing::{info, warn};

/// Indexing pipeline using Swiftide
pub struct IndexingPipeline {
    ollama_client: Ollama,
    qdrant_url: String,
    chunk_size: usize,
    fastembed_sparse: FastEmbed,
    fastembed_dense: FastEmbed,
    redis_cache: Redis,  // ← Add Redis cache
}

impl IndexingPipeline {
    /// Create a new indexing pipeline from configuration
    /// Create a new indexing pipeline with hybrid search support
     pub fn new(config: &SystemConfig) -> Result<Self> {
        let ollama_client = Ollama::builder()
            .default_embed_model("nomic-embed-text")
            .build()?;

        let fastembed_dense = FastEmbed::try_default()?.to_owned();
        let fastembed_sparse = FastEmbed::try_default_sparse()?.to_owned();

        // Initialize Redis for caching indexed nodes
        let redis_cache = Redis::try_from_url(
            &config.storage.redis_url.clone().unwrap_or_else(|| "redis://localhost:6379".to_string()),
            "swiftide-indexing"  // Cache namespace
        )?;

        Ok(Self {
            ollama_client,
            fastembed_dense,
            fastembed_sparse,
            redis_cache,
            qdrant_url: config.storage.qdrant_url.clone(),
            chunk_size: config.indexing.chunk_size,
        })
    }

    /// Index a single file with Redis-based deduplication
/// Index a file with automatic upsert (updates existing points by ID)
    pub async fn index_file(&self, file_path: &Path, tier: CollectionTier) -> Result<()> {
        let collection = tier.collection_name();

        info!("Indexing file: {} → {}", file_path.display(), collection);

        // Qdrant builder with upsert enabled (default behavior)
        let qdrant = SwiftideQdrant::try_from_url(&self.qdrant_url)?
            .batch_size(50)
            .vector_size(384)
            .with_vector(EmbeddedField::Combined)
            .with_sparse_vector(EmbeddedField::Combined)
            .collection_name(collection)
            .build()?;

        let is_code = self.is_code_file(file_path);
        let mut pipeline = Pipeline::from_loader(FileLoader::new(file_path));

        if is_code {
            let language = self.detect_language(file_path)?;  // ✅ Unwrap to String
            pipeline = pipeline
                // 1. Chunk with tree-sitter (generates stable chunk IDs)
                .then_chunk(ChunkCode::try_for_language_and_chunk_size(language, 10..self.chunk_size)?)
                // 2. Filter cached (skip unchanged chunks via Redis)
                .filter_cached(self.redis_cache.clone())
                // 3. Q&A metadata
                .then(MetadataQACode::from_client(self.ollama_client.clone()).build()?)
                // 4. Sparse embeddings
                .then_in_batch(
                    transformers::SparseEmbed::new(self.fastembed_sparse.clone())
                        .with_batch_size(32)
                )
                // 5. Dense embeddings
                .then_in_batch(
                    transformers::Embed::new(self.fastembed_dense.clone())
                        .with_batch_size(32)
                )
                // 6. Upsert to Qdrant (automatic - updates if ID exists, inserts if new)
                .then_store_with(qdrant);
        } else {
            pipeline = pipeline
                .then_chunk(ChunkMarkdown::from_chunk_range(10..self.chunk_size))
                .filter_cached(self.redis_cache.clone())
                .then_in_batch(
                    transformers::SparseEmbed::new(self.fastembed_sparse.clone())
                        .with_batch_size(32)
                )
                .then_in_batch(
                    transformers::Embed::new(self.fastembed_dense.clone())
                        .with_batch_size(32)
                )
                .then_store_with(qdrant);
        }

        pipeline.run().await?;
        info!("✓ Indexed/updated: {}", file_path.display());
        Ok(())
    }

    /// Detect programming language from file extension
    pub fn detect_language(&self, path: &Path) -> Result<&str> {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .context("No file extension")?;

        Ok(match ext {
            "rs" => "rust",
            "py" => "python",
            "js" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            "go" => "go",
            "java" => "java",
            "c" | "cpp" | "cc" => "cpp",
            _ => "rust", // fallback
        })
    }

    pub async fn index_directory(
        &self,
        dir_path: &Path,
        tier: CollectionTier,
        extensions: &[&str],
    ) -> Result<()> {
        let collection = tier.collection_name();

        info!("Indexing directory: {} → {}", dir_path.display(), collection);

        let mut loader = FileLoader::new(dir_path);
        if !extensions.is_empty() {
            loader = loader.with_extensions(extensions);
        }

        let language = self.detect_language(dir_path)?;  // ✅ Unwrap to String
        let qdrant = SwiftideQdrant::try_from_url(&self.qdrant_url)?
            .batch_size(50)
            .vector_size(384)
            .with_vector(EmbeddedField::Combined)
            .with_sparse_vector(EmbeddedField::Combined)
            .collection_name(collection)
            .build()?;

        Pipeline::from_loader(loader)
            .then_chunk(ChunkCode::try_for_language_and_chunk_size(language, 10..self.chunk_size)?)
            .filter_cached(self.redis_cache.clone())
            .then(MetadataQACode::from_client(self.ollama_client.clone()).build()?)
            .then_in_batch(
                transformers::SparseEmbed::new(self.fastembed_sparse.clone())
                    .with_batch_size(32)
            )
            .then_in_batch(
                transformers::Embed::new(self.fastembed_dense.clone())
                    .with_batch_size(32)
            )
            .then_store_with(qdrant)
            .run()
            .await?;

        info!("✓ Indexed directory: {}", dir_path.display());
        Ok(())
    }

    /// Batch index multiple files
    pub async fn index_batch(
        &self,
        files: Vec<(PathBuf, CollectionTier)>,
    ) -> Result<Vec<Result<()>>> {
        let mut results = Vec::new();

        for (file_path, tier) in files {
            let result = self.index_file(&file_path, tier).await;
            results.push(result);
        }

        Ok(results)
    }

    /// Re-index a file (delete old, index new)
    pub async fn reindex_file(
        &self,
        file_path: &Path,
        tier: CollectionTier,
        _qdrant_client: &QdrantClient,
    ) -> Result<()> {
        // Swiftide/Qdrant handles duplicates via point ID
        self.index_file(file_path, tier).await
    }

    /// Check if file is a code file
    pub fn is_code_file(&self, path: &Path) -> bool {
        let code_extensions = [
            "rs", "py", "js", "ts", "jsx", "tsx",
            "c", "cpp", "h", "hpp", "go", "java",
            "rb", "php", "swift", "kt", "scala",
        ];

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| code_extensions.contains(&ext))
            .unwrap_or(false)
    }
}

/// Indexing coordinator that watches files and manages the pipeline
pub struct IndexingCoordinator {
    pipeline: IndexingPipeline,
    config: SystemConfig,
}

impl IndexingCoordinator {
    pub fn new(config: SystemConfig) -> Result<Self> {
        Ok(Self {
            pipeline: IndexingPipeline::new(&config)?,
            config,
        })
    }

    /// Handle a file system event
    pub async fn handle_file_event(
        &self,
        path: &Path,
        event_type: &str,
        tier: CollectionTier,
    ) -> Result<()> {
        match event_type {
            "created" | "modified" => {
                self.pipeline.index_file(path, tier).await?;
            }
            "deleted" => {
                // TODO: Implement deletion from Qdrant
                warn!("File deleted, but deletion from vector store not yet implemented: {}", path.display());
            }
            _ => {
                warn!("Unknown event type: {}", event_type);
            }
        }
        Ok(())
    }

    /// Initial indexing of all configured paths
    pub async fn initial_index(&self) -> Result<()> {
        info!("Starting initial indexing...");

        // Index workspace paths
        for path in &self.config.indexing.workspace_paths {
            self.pipeline
                .index_directory(path, CollectionTier::Workspace, &["rs", "py", "js", "ts", "md"])
                .await?;
        }

        // Index personal paths
        for path in &self.config.indexing.personal_paths {
            self.pipeline
                .index_directory(path, CollectionTier::Personal, &["md", "txt", "org"])
                .await?;
        }

        // Index system paths
        for path in &self.config.indexing.system_paths {
            self.pipeline
                .index_directory(path, CollectionTier::System, &[])
                .await?;
        }

        info!("Initial indexing complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn test_config() -> SystemConfig {
        SystemConfig {
            indexing: IndexingConfig {
                workspace_paths: vec![],
                personal_paths: vec![],
                system_paths: vec![],
                watch_enabled: true,
                chunk_size: 512,
                filters: IndexingFilters::default(),
            },
            rag: SystemConfig::default().rag,
            orchestrator: SystemConfig::default().orchestrator,
            storage: StorageConfig {
                qdrant_url: "http://localhost:6333".to_string(),
                postgres_url: "postgresql://localhost/test".to_string(),
                redis_url: None,
            },
        }
    }

    #[test]
    fn test_pipeline_creation() {
        let config = test_config();
        let pipeline = IndexingPipeline::new(&config);

        assert!(pipeline.is_ok());
    }

    #[test]
    fn test_is_code_file() {
        let config = test_config();
        let pipeline = IndexingPipeline::new(&config).unwrap();

        assert!(pipeline.is_code_file(Path::new("main.rs")));
        assert!(pipeline.is_code_file(Path::new("app.py")));
        assert!(pipeline.is_code_file(Path::new("script.js")));
        assert!(!pipeline.is_code_file(Path::new("README.md")));
        assert!(!pipeline.is_code_file(Path::new("notes.txt")));
    }

    // Full integration tests require Ollama and Qdrant running
    #[tokio::test]
    #[ignore]
    async fn test_index_file_integration() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.rs");
        fs::write(&test_file, "fn main() { println!(\"test\"); }").unwrap();

        let config = test_config();
        let pipeline = IndexingPipeline::new(&config).unwrap();

        let result = pipeline
            .index_file(&test_file, CollectionTier::Workspace)
            .await;

        assert!(result.is_ok());
    }
}
