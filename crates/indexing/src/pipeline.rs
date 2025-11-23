use ai_agent_common::{llm::EmbeddingClient, types::*, config::{IndexingConfig, SystemConfig}};
use ai_agent_storage::QdrantClient;
use anyhow::{Context, Result, anyhow};
use repo_root::{projects::GitProject, RepoRoot};
use serde_json::json;
use std::{fs, path::Path, sync::Arc};
use crate::{chunk_adaptive::ChunkAdaptive, metadata_chunk_transformer::ExtractMetadataChunkTransformer, metadata_transformer::{ExtractMetadataTransformer}};
// Add all the tree-sitter language crates you want to support
use tree_sitter::{self, Parser, Tree};

// Correct Swiftide 0.32 imports
use swiftide::indexing::{
    loaders::FileLoader, transformers::{self,  MetadataQACode}, EmbedMode, Pipeline
};
use swiftide::integrations::{
    ollama::Ollama,
    redis::Redis,
};

use tracing::{info, warn, debug, error};

/// Indexing pipeline using Swiftide
pub struct IndexingPipeline {
    qdrant_client: Arc<QdrantClient>,
    redis_cache: Redis,
    config: IndexingConfig,
    embedder: Arc<EmbeddingClient>,
}

impl IndexingPipeline {
    /// Create a new indexing pipeline from configuration
    /// Create a new indexing pipeline with hybrid search support
    pub fn new(config: &SystemConfig, embedder: Arc<EmbeddingClient>) -> Result<Self> {
        debug!("Creating IndexingPipeline");


        debug!("Creating Redis cache...");
        let redis_cache = Redis::try_from_url(
            &config.storage.redis_url
                .clone()
                .unwrap_or_else(|| "redis://localhost:6379".to_string()),
            "swiftide-indexing"
        ).context("Failed to create Redis cache")?;

        debug!("Initializing Qdrant client...");
        let qdrant_client = Arc::new(QdrantClient::new(&config.storage.qdrant_url.to_string(),embedder.clone())
            .context("Failed to create Qdrant client")?);

        // Initialize Redis for caching indexed nodes

        Ok(Self {
            redis_cache,
            qdrant_client,
            config: config.indexing.clone(),
            embedder: embedder
        })
    }

    /// Index a single file with Redis-based deduplication
/// Index a file with automatic upsert (updates existing points by ID)
    pub async fn index_directory(&self, file_path: &Path, tier: CollectionTier, extensions: Option<&Vec<&str>>) -> Result<()> {
        let collection = tier.to_string();

        info!("Indexing file: {} → {}", file_path.display(), collection);

        // Qdrant builder with upsert enabled (default behavior)
        let qdrant = self.qdrant_client.indexing_client(&collection)?;

        let pipeline = self.create_pipeline(file_path,extensions)
            .map_err(|err| {
                error!("Failed to create pipeline: {:?}", err);
                err
            })?;

        pipeline
            .then_store_with(qdrant)
            .run()
            .await?;
        // Explicitly unload Ollama
        // let client = reqwest::Client::new();
        // client.post("http://localhost:11434/api/generate")
        //     .json(&json!({"model": "all-minilm", "keep_alive": 0}))
        //     .send()
        //     .await?;
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


    fn create_pipeline(&self, path: &Path, extensions: Option<&Vec<&str>>,
) -> Result<Pipeline<String>>{


        let root_path = RepoRoot::<GitProject>::new(&path).path;
        let project_root = root_path.to_str().unwrap().to_string();

        let metadatatransformer = ExtractMetadataTransformer::new(project_root);


        // Example custom transformer to add useful metadata
        let dense_embedding_model = self.embedder.embedder_dense.clone();
        debug!("Initializing FastEmbed sparse...");
        let sparse_embedding_model = self.embedder.embedder_sparse.clone();

        let mut file_loader = FileLoader::new(path);
        if let Some(ext) = extensions{
            file_loader = file_loader.with_extensions(ext);
        }
        let pipeline = Pipeline::from_loader(file_loader)
            .with_concurrency(1)
            .with_embed_mode(EmbedMode::PerField);
        let mut meta_pipeline = pipeline
            // .filter_cached(self.redis_cache.clone())
            // .then(MetadataTitle::new(prompt_client.clone()))
            // .then(MetadataSummary::new(prompt_client.clone()))
            // .then(MetadataKeywords::new(prompt_client.clone()))
            .then(metadatatransformer);

        if self.config.enable_qa_metadata{
            let prompt_client = Ollama::builder()
            .default_prompt_model("llama3.1:8b")
            .build()
            .context("Failed to build Ollama prompt client")?;
            meta_pipeline = meta_pipeline.then(MetadataQACode::from_client(prompt_client).build()?)
        }
        let chunked_pipeline = meta_pipeline.then_chunk(ChunkAdaptive::default())

        .then(ExtractMetadataChunkTransformer::new())
        // 5. Dense embeddings
        .then_in_batch(
            transformers::Embed::new(dense_embedding_model)
                .with_batch_size(self.config.batch_size)
        )
        // 4. Sparse embeddings
        .then_in_batch(
            transformers::SparseEmbed::new(sparse_embedding_model)
                .with_batch_size(self.config.batch_size)
        );
        // .log_errors();
        Ok(chunked_pipeline)
    }

    /// Batch index multiple files
    // pub async fn index_batch(
    //     &self,
    //     files: Vec<(PathBuf, CollectionTier)>,
    // ) -> Result<Vec<Result<()>>> {
    //     let mut results = Vec::new();
    //
    //     for (file_path, tier) in files {
    //         let result = self.index_directory(&file_path, tier).await;
    //         results.push(result);
    //     }
    //
    //     Ok(results)
    // }

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
    pub fn new(config: SystemConfig, embedder: Arc<EmbeddingClient>) -> Result<Self> {
        debug!("Creating IndexingCoordinator");
        debug!("Qdrant URL: {}", config.storage.qdrant_url);

        debug!("Creating IndexingPipeline...");
        let pipeline = IndexingPipeline::new(&config, embedder)
            .context("Failed to create indexing pipeline")?;

        debug!("IndexingCoordinator created successfully");
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
                self.pipeline.index_directory(path, tier,None).await?;
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
            let extensions =  vec!["rs", "py", "js", "ts", "md"];
            self.pipeline
                .index_directory(path, CollectionTier::Workspace, Some(&extensions))
                .await?;
        }

        // Index personal paths
        for path in &self.config.indexing.personal_paths {
            let extensions =   vec!["md", "txt", "org"];
            self.pipeline
                .index_directory(path, CollectionTier::Personal, Some(&extensions))
                .await?;
        }

        // Index system paths
        for path in &self.config.indexing.system_paths {
            self.pipeline
                .index_directory(path, CollectionTier::System,None)
                .await?;
        }

        info!("Initial indexing complete");
        Ok(())
    }
}
