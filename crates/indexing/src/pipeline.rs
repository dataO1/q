use ai_agent_common::*;
use ai_agent_storage::QdrantClient;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use crate::chunk_adaptive::ChunkAdaptive;

// Correct Swiftide 0.32 imports
use swiftide::indexing::{
    loaders::FileLoader,
    transformers::{self,  MetadataQACode},
    Pipeline,
};
use swiftide::integrations::{
    ollama::Ollama,
    fastembed::FastEmbed,
    redis::Redis,
};

use tracing::{info, warn};

/// Indexing pipeline using Swiftide
pub struct IndexingPipeline {
    ollama_client: Ollama,
    qdrant_client: QdrantClient,
    fastembed_sparse: FastEmbed,
    fastembed_dense: FastEmbed,
    redis_cache: Redis,  // ← Add Redis cache
    config: IndexingConfig//
}

impl IndexingPipeline {
    /// Create a new indexing pipeline from configuration
    /// Create a new indexing pipeline with hybrid search support
     pub fn new(config: &SystemConfig) -> Result<Self> {
        tracing::debug!("Creating IndexingPipeline");

        tracing::debug!("Building Ollama client...");
        let ollama_client = Ollama::builder()
            .default_embed_model("nomic-embed-text")
            .default_prompt_model("qwen2.5-coder:7b")
            .build()
            .context("Failed to build Ollama client")?;

        tracing::debug!("Initializing FastEmbed dense...");
        let fastembed_dense = FastEmbed::try_default()
            .context("Failed to initialize dense embedder")?
            .to_owned();

        tracing::debug!("Initializing FastEmbed sparse...");
        let fastembed_sparse = FastEmbed::try_default_sparse()
            .context("Failed to initialize sparse embedder")?
            .to_owned();

        tracing::debug!("Creating Redis cache...");
        let redis_cache = Redis::try_from_url(
            &config.storage.redis_url
                .clone()
                .unwrap_or_else(|| "redis://localhost:6379".to_string()),
            "swiftide-indexing"
        ).context("Failed to create Redis cache")?;

        tracing::debug!("Initializing Qdrant client...");
        let qdrant_client = QdrantClient::new(&config.storage.qdrant_url.to_string())
            .context("Failed to create Qdrant client")?;

        // Initialize Redis for caching indexed nodes

        Ok(Self {
            ollama_client,
            fastembed_dense,
            fastembed_sparse,
            redis_cache,
            qdrant_client,
            config: config.indexing.clone()
        })
    }

    /// Index a single file with Redis-based deduplication
/// Index a file with automatic upsert (updates existing points by ID)
    pub async fn index_directory(&self, file_path: &Path, tier: CollectionTier) -> Result<()> {
        let collection = tier.collection_name();

        info!("Indexing file: {} → {}", file_path.display(), collection);

        // Qdrant builder with upsert enabled (default behavior)
        let qdrant = self.qdrant_client.indexing_client(&collection)?;

        let pipeline = self.create_pipeline(file_path,&["rs", "md"])
            .map_err(|err| {
                tracing::error!("Failed to create pipeline: {:?}", err);
                err
            })?;

        pipeline
            .then_store_with(qdrant)
            .run()
            .await?;
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

    fn create_pipeline(&self, path: &Path, extensions: &[&str],
) -> Result<Pipeline<String>>{

        let file_loader = FileLoader::new(path).with_extensions(extensions);
        let mut pipeline = Pipeline::from_loader(file_loader);
        pipeline = pipeline
            .then_chunk(ChunkAdaptive::default());
            // 2. Filter cached (skip unchanged chunks via Redis)
            // .filter_cached(self.redis_cache.clone());
            // 3. Q&A metadata
            if self.config.enable_qa_metadata{
                pipeline = pipeline.then(MetadataQACode::from_client(self.ollama_client.clone()).build()?)
            }
            // 4. Sparse embeddings
            pipeline = pipeline.then_in_batch(
                transformers::SparseEmbed::new(self.fastembed_sparse.clone())
                    .with_batch_size(32)
            )
            // 5. Dense embeddings
            .then_in_batch(
                transformers::Embed::new(self.fastembed_dense.clone())
                    .with_batch_size(32)
            );
        Ok(pipeline)
    }

    /// Batch index multiple files
    pub async fn index_batch(
        &self,
        files: Vec<(PathBuf, CollectionTier)>,
    ) -> Result<Vec<Result<()>>> {
        let mut results = Vec::new();

        for (file_path, tier) in files {
            let result = self.index_directory(&file_path, tier).await;
            results.push(result);
        }

        Ok(results)
    }

    /// Check if file is a code file
    pub fn is_code_file(&self, path: &Path) -> bool {
        let code_extensions = [
            "rs", "py", "js", "ts", "jsx", "tsx",
            "c", "cpp", "h", "hpp", "go", "java",
            "rb", "php", "swift", "kt", "scala",
        ];

        path.is_file() && path.extension()
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
        tracing::debug!("Creating IndexingCoordinator");
        tracing::debug!("Qdrant URL: {}", config.storage.qdrant_url);

        tracing::debug!("Creating IndexingPipeline...");
        let pipeline = IndexingPipeline::new(&config)
            .context("Failed to create indexing pipeline")?;

        tracing::debug!("IndexingCoordinator created successfully");
        Ok(Self { config, pipeline })
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
                self.pipeline.index_directory(path, tier).await?;
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
            let extensions =  &["rs", "py", "js", "ts", "md"];
            self.pipeline
                .index_directory(path, CollectionTier::Workspace)
                .await?;
        }

        // Index personal paths
        for path in &self.config.indexing.personal_paths {
            let extensions =   &["md", "txt", "org"];
            self.pipeline
                .index_directory(path, CollectionTier::Personal)
                .await?;
        }

        // Index system paths
        for path in &self.config.indexing.system_paths {
            self.pipeline
                .index_directory(path, CollectionTier::System)
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
            indexing: IndexingConfig::default(),
            rag: SystemConfig::default().rag,
            orchestrator: SystemConfig::default().orchestrator,
            storage: StorageConfig {
                qdrant_url: "http://localhost:16334".to_string(),
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
            .index_directory(&test_file, CollectionTier::Workspace)
            .await;

        assert!(result.is_ok());
    }
}
