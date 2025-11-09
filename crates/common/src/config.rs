use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Context, Result};

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
    /// Load configuration from TOML file
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context(format!("Failed to read config file: {}", path))?;

        let config: SystemConfig = toml::from_str(&content)
            .context("Failed to parse TOML configuration")?;

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Validate indexing config
        if self.indexing.chunk_size == 0 {
            anyhow::bail!("chunk_size must be greater than 0");
        }
        if self.indexing.chunk_size > 4096 {
            anyhow::bail!("chunk_size too large (max 4096)");
        }

        // Validate RAG weights
        if self.rag.max_results == 0 {
            anyhow::bail!("max_results must be greater than 0");
        }

        // Validate agent configs
        if self.orchestrator.agents.is_empty() {
            anyhow::bail!("At least one agent must be configured");
        }

        for agent in &self.orchestrator.agents {
            if agent.temperature < 0.0 || agent.temperature > 2.0 {
                anyhow::bail!(
                    "Invalid temperature {} for agent {}. Must be between 0.0 and 2.0",
                    agent.temperature,
                    agent.name
                );
            }
        }

        // Validate storage URLs
        if !self.storage.qdrant_url.starts_with("http") {
            anyhow::bail!("qdrant_url must be a valid HTTP URL");
        }
        if !self.storage.postgres_url.starts_with("postgresql") {
            anyhow::bail!("postgres_url must be a valid PostgreSQL connection string");
        }

        Ok(())
    }

    /// Get agent config by name
    pub fn get_agent_config(&self, name: &str) -> Option<&AgentConfig> {
        self.orchestrator.agents.iter().find(|a| a.name == name)
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            indexing: IndexingConfig {
                workspace_paths: vec![],
                personal_paths: vec![],
                system_paths: vec![],
                watch_enabled: true,
                chunk_size: 512,
            },
            rag: RagConfig {
                reranking_weights: RerankingWeights {
                    conversation_boost: 1.5,
                    recency_boost: 1.2,
                    dependency_boost: 1.3,
                },
                query_enhancement_model: "qwen2.5:7b".to_string(),
                max_results: 5,
            },
            orchestrator: OrchestratorConfig {
                agents: vec![],
                checkpoint_interval: "after_wave".to_string(),
            },
            storage: StorageConfig {
                qdrant_url: "http://localhost:6333".to_string(),
                postgres_url: "postgresql://localhost/ai_agent".to_string(),
                redis_url: Some("redis://localhost:6379".to_string()),
            },
        }
    }
}
