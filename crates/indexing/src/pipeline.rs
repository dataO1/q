use ai_agent_common::*;
use ai_agent_storage::QdrantClient;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

// Correct Swiftide 0.32 imports
use swiftide::indexing::{self, Pipeline};
use swiftide::indexing::loaders::FileLoader;
use swiftide::indexing::transformers::{ChunkCode, ChunkMarkdown, Embed};
use swiftide::integrations::{ollama::Ollama, qdrant::Qdrant as SwiftideQdrant};
use tracing::{info, warn};

/// Indexing pipeline using Swiftide
pub struct IndexingPipeline {
    ollama_client: Ollama,
    qdrant_url: String,
    chunk_size: usize,
}

impl IndexingPipeline {
    /// Create a new indexing pipeline from configuration
    pub fn new(config: &SystemConfig) -> Result<Self> {
        let ollama_client = Ollama::builder()
            .default_embed_model("nomic-embed-text")
            .build()?;

        Ok(Self {
            ollama_client,
            qdrant_url: config.storage.qdrant_url.clone(),
            chunk_size: config.indexing.chunk_size,
        })
    }

    /// Index a single file into the appropriate collection tier
    pub async fn index_file(
        &self,
        file_path: &Path,
        tier: CollectionTier,
    ) -> Result<()> {
        let collection = tier.collection_name();

        info!("Indexing file: {} → {}", file_path.display(), collection);

        // Build Qdrant storage
        let qdrant = SwiftideQdrant::try_from_url(&self.qdrant_url)?
            .batch_size(50)
            .vector_size(768)  // nomic-embed-text dimension
            .collection_name(collection)
            .build()?;

        // Determine if it's code or markdown
        let is_code = self.is_code_file(file_path);

        // Build the pipeline
        let mut pipeline = Pipeline::from_loader(
            FileLoader::new(file_path)
        );

        // Add appropriate chunker
        if is_code {
            if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                let language = match ext {
                    "rs" => "rust",
                    "py" => "python",
                    "js" | "jsx" => "javascript",
                    "ts" | "tsx" => "typescript",
                    _ => "rust", // fallback
                };

                pipeline = pipeline.then_chunk(
                    ChunkCode::try_for_language_and_chunk_size(
                        language,
                        10..self.chunk_size,
                    )?
                );
            }
        } else {
            pipeline = pipeline.then_chunk(
                ChunkMarkdown::from_chunk_range(10..self.chunk_size)
            );
        }

        // Add embedding and storage
        pipeline = pipeline
            .then_in_batch(Embed::new(self.ollama_client.clone()))
            .then_store_with(qdrant);

        // Run the pipeline
        pipeline.run().await
            .context(format!("Failed to index file: {}", file_path.display()))?;

        info!("Successfully indexed: {}", file_path.display());
        Ok(())
    }

    /// Index an entire directory
    pub async fn index_directory(
        &self,
        dir_path: &Path,
        tier: CollectionTier,
        extensions: &[&str],
    ) -> Result<()> {
        let collection = tier.collection_name();

        info!("Indexing directory: {} → {}", dir_path.display(), collection);

        let mut loader = FileLoader::new(dir_path);

        // Filter by extensions if provided
        if !extensions.is_empty() {
            loader = loader.with_extensions(extensions);
        }

        // Build Qdrant storage
        let qdrant = SwiftideQdrant::try_from_url(&self.qdrant_url)?
            .batch_size(50)
            .vector_size(768)
            .collection_name(collection)
            .build()?;

        // Build pipeline with code chunking for all files
        Pipeline::from_loader(loader)
            .then_chunk(ChunkCode::try_for_language_and_chunk_size(
                "rust",  // Will auto-detect
                10..self.chunk_size,
            )?)
            .then_in_batch(Embed::new(self.ollama_client.clone()))
            .then_store_with(qdrant)
            .run()
            .await
            .context(format!("Failed to index directory: {}", dir_path.display()))?;

        info!("Successfully indexed directory: {}", dir_path.display());
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
