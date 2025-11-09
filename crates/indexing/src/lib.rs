//! Indexing pipeline with file watching and smart chunking

pub mod watcher;
pub mod classifier;
pub mod chunker;
pub mod embedder;
pub mod storage;

use ai_agent_common::*;

/// Main indexing pipeline
pub struct IndexingPipeline {
    classifier: classifier::PathClassifier,
    chunker: chunker::SmartChunker,
    embedder: embedder::OllamaEmbedder,
    storage: storage::QdrantStorage,
}

impl IndexingPipeline {
    pub async fn new(config: &IndexingConfig) -> Result<Self> {
        todo!("Initialize indexing pipeline")
    }

    pub async fn index_file(&self, path: &std::path::Path) -> Result<()> {
        todo!("Index single file through pipeline")
    }

    pub async fn index_directory(&self, path: &std::path::Path) -> Result<()> {
        todo!("Recursively index directory")
    }
}
