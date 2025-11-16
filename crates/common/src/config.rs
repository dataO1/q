use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use anyhow::{Context, Result};

use crate::HitlMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub indexing: IndexingConfig,
    pub rag: RagConfig,
    pub storage: StorageConfig,
    pub embedding: EmbeddingConfig,
    pub agent_network: AgentNetworkConfig,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub ollama_host: String,
    pub ollama_port: u16,
    pub dense_model: String,
    pub vector_size: u64,
}
impl EmbeddingConfig {
    fn default() -> EmbeddingConfig {
         return Self{
            ollama_host:"http://localhost".to_string(),
            ollama_port:11434,
            dense_model : "jeffh/intfloat-e5-base-v2:f32".to_string(),
            vector_size: 786,
         }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    pub workspace_paths: Vec<PathBuf>,
    pub personal_paths: Vec<PathBuf>,
    pub system_paths: Vec<PathBuf>,
    pub watch_enabled: bool,
    #[serde(default)]
    pub filters: IndexingFilters,
    pub enable_qa_metadata: bool,
    pub batch_size: usize,
}

/// File filtering configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingFilters {
    /// Respect .gitignore files
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,

    /// Include hidden files (starting with .)
    #[serde(default)]
    pub include_hidden: bool,

    /// Additional directories to ignore (beyond .gitignore)
    #[serde(default = "default_ignore_dirs")]
    pub ignore_dirs: Vec<String>,

    /// File extensions to ignore
    #[serde(default)]
    pub ignore_extensions: Vec<String>,

    /// Custom ignore patterns (gitignore syntax)
    #[serde(default)]
    pub custom_ignores: Vec<String>,

    /// File size limit in bytes (None = no limit)
    #[serde(default)]
    pub max_file_size: Option<u64>,
}

impl Default for IndexingConfig{

    fn default() -> Self {
        Self {
            workspace_paths: vec![],
            personal_paths: vec![],
            system_paths: vec![],
            watch_enabled: true,
            batch_size: 32,
            filters: IndexingFilters::default(),
            enable_qa_metadata: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_ignore_dirs() -> Vec<String> {
    vec![
        "target".to_string(),
        "node_modules".to_string(),
        "build".to_string(),
        "dist".to_string(),
        ".git".to_string(),
        ".svn".to_string(),
        "__pycache__".to_string(),
        ".cache".to_string(),
        "venv".to_string(),
        ".venv".to_string(),
    ]
}

impl Default for IndexingFilters {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            include_hidden: false,
            ignore_dirs: default_ignore_dirs(),
            ignore_extensions: vec![],
            custom_ignores: vec![],
            max_file_size: Some(10 * 1024 * 1024), // 10MB default
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    pub reranking_weights: RerankingWeights,
    pub query_enhancement_model: String,
    pub classification_model: String,
    pub max_results: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankingWeights {
    pub conversation_boost: f32,
    pub recency_boost: f32,
    pub dependency_boost: f32,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub qdrant_url: String,
    pub postgres_url: String,
    pub redis_url: Option<String>,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentNetworkConfig {
    pub agents: Vec<AgentConfig>,
    pub hitl: HitlConfig,
    pub retry: RetryConfig,
    pub token_budget: TokenBudgetConfig,
    pub acp: AcpConfig,
    pub tracing: TracingConfig,
}

impl Default for AgentNetworkConfig {
    fn default() -> Self {
        Self {
            agents: vec![],
            hitl: HitlConfig::default(),
            retry: RetryConfig::default(),
            token_budget: TokenBudgetConfig::default(),
            acp: AcpConfig::default(),
            tracing: TracingConfig::default(),
        }
    }
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub id: String,
    pub agent_type: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HitlConfig {
    pub enabled: bool,
    pub default_mode: HitlMode,
    pub sample_rate: Option<f32>,
}


impl Default for HitlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_mode: HitlMode::Blocking,
            sample_rate: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub backoff_ms: u64,
    pub backoff_multiplier: f32,
}


impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            backoff_ms: 9999999,
            backoff_multiplier: 1f32,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenBudgetConfig {
    pub max_tokens_per_agent: usize,
    pub enable_context_pruning: bool,
    pub enable_prompt_caching: bool,
}


impl Default for TokenBudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_agent: 4096,
            enable_context_pruning: true,
            enable_prompt_caching: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AcpConfig {
    pub host: String,
    pub port: u16,
}

impl Default for AcpConfig {
    fn default() -> Self {
        Self {
            host : "0.0.0.0".to_string(),
            port : 8080
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TracingConfig {
    pub enabled: bool,
    pub jaeger_endpoint: Option<String>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled : true,
            jaeger_endpoint : Some("http://localhost:14268/api/traces".to_string())
        }
    }
}


impl SystemConfig {
    pub fn new(indexing: IndexingConfig, rag: RagConfig,agent_network: AgentNetworkConfig,  storage: StorageConfig, embedding: EmbeddingConfig) -> Self {
        Self { indexing, rag,agent_network, storage, embedding}
    }

    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .context(format!("Failed to read config file: {}", path))?;

        let config: SystemConfig = toml::from_str(&contents)
            .context("Failed to parse config file")?;

        Ok(config)
    }
    // Load configuration from file
    pub fn load_config(path: &str) -> Result<SystemConfig> {
        SystemConfig::from_file(path)
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Validate indexing config
        if self.indexing.batch_size == 0 {
            anyhow::bail!("chunk_size must be greater than 0");
        }
        if self.indexing.batch_size > 4096 {
            anyhow::bail!("chunk_size too large (max 4096)");
        }

        // Validate RAG weights
        if self.rag.max_results == 0 {
            anyhow::bail!("max_results must be greater than 0");
        }

        // Validate agent configs
        if self.agent_network.agents.is_empty() {
            anyhow::bail!("At least one agent must be configured");
        }

        for agent in &self.agent_network.agents {
            if agent.temperature < 0.0 || agent.temperature > 2.0 {
                anyhow::bail!(
                    "Invalid temperature {} for agent {}. Must be between 0.0 and 2.0",
                    agent.temperature,
                    agent.id
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
        self.agent_network.agents.iter().find(|a| a.id == name)
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
                batch_size: 32,
                filters: IndexingFilters::default(),
                enable_qa_metadata: false,
            },
            rag: RagConfig {
                reranking_weights: RerankingWeights {
                    conversation_boost: 1.5,
                    recency_boost: 1.2,
                    dependency_boost: 1.3,
                },
                query_enhancement_model: "qwen2.5:7b".to_string(),
                classification_model: "phi3:mini".to_string(),
                max_results: 5,
            },
            agent_network: AgentNetworkConfig::default() ,
            storage: StorageConfig {
                qdrant_url: "http://localhost:6333".to_string(),
                postgres_url: "postgresql://localhost/ai_agent".to_string(),
                redis_url: Some("redis://localhost:6379".to_string()),
            },
            embedding: EmbeddingConfig::default()
        }
    }
}


