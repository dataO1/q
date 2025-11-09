use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub indexing: IndexingConfig,
    pub rag: RagConfig,
    pub orchestrator: OrchestratorConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    pub workspace_paths: Vec<PathBuf>,
    pub personal_paths: Vec<PathBuf>,
    pub system_paths: Vec<PathBuf>,
    pub watch_enabled: bool,
    pub chunk_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    pub reranking_weights: RerankingWeights,
    pub query_enhancement_model: String,
    pub max_results: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankingWeights {
    pub conversation_boost: f32,
    pub recency_boost: f32,
    pub dependency_boost: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    pub agents: Vec<AgentConfig>,
    pub checkpoint_interval: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub model: String,
    pub system_prompt: String,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub qdrant_url: String,
    pub postgres_url: String,
    pub redis_url: Option<String>,
}

impl SystemConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: SystemConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
