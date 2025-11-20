use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use std::{fs, path::PathBuf};
use anyhow::{Context, anyhow};

use crate::{AgentType, ErrorRecoveryStrategy, HitlMode, QualityStrategy};

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
    pub quality: QualityConfig,
}

impl AgentNetworkConfig {
    /// Validate configuration
    fn validate(&self) -> anyhow::Result<()> {
        // Validate agents
        if self.agents.is_empty() {
            return Err(anyhow!("At least one agent must be configured".to_string()));
        }

        let mut agent_ids = std::collections::HashSet::new();
        for agent in &self.agents {
            // Check for duplicate IDs
            if !agent_ids.insert(&agent.id) {
                return Err(anyhow!("Duplicate agent ID: {}", agent.id));
            }

            // Validate agent fields
            agent.validate()?;
        }

        // Validate retry configuration
        if self.retry.max_attempts == 0 {
            warn!("max_attempts is 0, retries will be disabled");
        }

        // Validate token budget
        if self.token_budget.max_tokens_per_agent == 0 {
            return Err(anyhow!("max_tokens_per_agent must be greater than 0".to_string()));
        }

        // Validate pruning threshold
        if !(0.0..=1.0).contains(&self.token_budget.pruning_threshold) {
            return Err(anyhow!("pruning_threshold must be between 0.0 and 1.0".to_string()));
        }

        // Validate HITL settings
        if !(0.0..=1.0).contains(&self.hitl.sample_rate) {
            return Err(anyhow!("sample_rate must be between 0.0 and 1.0".to_string()));
        }

        // Validate quality settings
        if !(0.0..=1.0).contains(&self.quality.min_quality_score) {
            return Err(anyhow!("min_quality_score must be between 0.0 and 1.0".to_string()));
        }

        Ok(())
    }

    /// Get agent by ID
    pub fn get_agent(&self, agent_id: &str) -> Option<&AgentConfig> {
        self.agents.iter().find(|a| a.id == agent_id)
    }

    /// Get agents by type
    pub fn get_agents_by_type(&self, agent_type: AgentType) -> Vec<&AgentConfig> {
        self.agents
            .iter()
            .filter(|a| a.agent_type == agent_type)
            .collect()
    }

    /// Get all available agent types
    pub fn available_agent_types(&self) -> Vec<AgentType> {
        let mut types: Vec<_> = self.agents.iter().map(|a| a.agent_type.clone()).collect();
        // types.sort();
        types.dedup();
        types
    }
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
            quality: QualityConfig::default(),
        }
    }
}


/// Individual agent configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    /// Unique agent identifier
    pub id: String,

    /// Agent type (coding, planning, writing, evaluator)
    pub agent_type: AgentType,

    /// LLM model identifier (e.g., "qwen2.5-coder:32b")
    pub model: String,

    /// Model temperature (0.0 - 2.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Maximum tokens for model output
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// System prompt for agent
    pub system_prompt: String,

    /// Error recovery strategy for this agent
    #[serde(default)]
    pub recovery_strategy: Option<ErrorRecoveryStrategy>,

    /// Quality evaluation strategy for this agent
    #[serde(default)]
    pub quality_strategy: Option<QualityStrategy>,

    /// Agent-specific context window size
    #[serde(default = "default_context_window")]
    pub context_window: usize,

    /// Enable streaming for this agent
    #[serde(default = "default_true")]
    pub enable_streaming: bool,

    /// Metadata for agent capabilities
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl AgentConfig {
    /// Validate agent configuration
    fn validate(&self) -> anyhow::Result<()> {
        if self.id.is_empty() {
            return Err(anyhow!("Agent ID cannot be empty".to_string()));
        }

        // if self.agent_type.is_empty() {
        //     return Err(anyhow!("Agent {} type cannot be empty", self.id));
        // }

        if self.model.is_empty() {
            return Err(anyhow!("Agent {} model cannot be empty", self.id));
        }

        if !(0.0..=2.0).contains(&self.temperature) {
            return Err(anyhow!( "Agent {} temperature must be between 0.0 and 2.0", self.id));
        }

        if self.max_tokens == 0 {
            return Err(anyhow!("Agent {} max_tokens must be greater than 0", self.id));
        }

        if self.system_prompt.is_empty() {
            return Err(anyhow!("Agent {} system_prompt cannot be empty", self.id));
        }

        Ok(())
    }

    /// Get effective quality strategy
    pub fn effective_quality_strategy(&self) -> Option<QualityStrategy> {
        self.quality_strategy
    }

    /// Get effective recovery strategy
    pub fn effective_recovery_strategy(&self) -> Option<ErrorRecoveryStrategy> {
        self.recovery_strategy.clone()
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for RiskLevel {
    fn default() -> Self{
        RiskLevel::High
    }
}

impl RiskLevel {
    pub fn from_confidence(conf: f32) -> Self {
        match conf {
            c if c >= 0.9 => RiskLevel::Low,
            c if c >= 0.75 => RiskLevel::Medium,
            c if c >= 0.60 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }
}


/// HITL (Human-in-the-Loop) configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct HitlConfig {
    /// Enable HITL system
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Default HITL mode
    #[serde(default)]
    pub mode: HitlMode,

    #[serde(default = "default_risk_threshold")]
    pub risk_threshold: RiskLevel,  // "low", "medium", "high", "critical"

    /// Sample rate for sample-based HITL (0.0 - 1.0)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f32,

    /// Timeout for HITL approvals (seconds)
    #[serde(default = "default_hitl_timeout")]
    pub approval_timeout_secs: u64,
}


/// Retry behavior configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    #[serde(default = "default_max_attempts")]
    pub max_attempts: usize,

    /// Initial backoff in milliseconds
    #[serde(default = "default_backoff_ms")]
    pub backoff_ms: u64,

    /// Exponential backoff multiplier
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f32,

    /// Maximum backoff cap (milliseconds)
    #[serde(default = "default_max_backoff_ms")]
    pub max_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            backoff_ms: default_backoff_ms(),
            backoff_multiplier: default_backoff_multiplier(),
            max_backoff_ms: default_max_backoff_ms(),
        }
    }
}


/// Token budget management configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenBudgetConfig {
    /// Maximum tokens per agent
    #[serde(default = "default_max_tokens_per_agent")]
    pub max_tokens_per_agent: usize,

    /// Enable context pruning
    #[serde(default = "default_true")]
    pub enable_context_pruning: bool,

    /// Enable prompt caching
    #[serde(default = "default_true")]
    pub enable_prompt_caching: bool,

    /// Pruning strategy threshold
    #[serde(default = "default_pruning_threshold")]
    pub pruning_threshold: f32,
}

impl Default for TokenBudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_agent: default_max_tokens_per_agent(),
            enable_context_pruning: default_true(),
            enable_prompt_caching: default_true(),
            pruning_threshold: default_pruning_threshold(),
        }
    }
}

/// ACP (Agent Communication Protocol) server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AcpConfig {
    /// Server hostname
    #[serde(default = "default_host")]
    pub host: String,

    /// Server port
    #[serde(default = "default_port")]
    pub port: u16,

    /// WebSocket enable
    #[serde(default = "default_true")]
    pub enable_websocket: bool,

    /// CORS allowed origins
    #[serde(default)]
    pub cors_origins: Vec<String>,

    /// Request timeout (seconds)
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,
}

impl Default for AcpConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            enable_websocket: default_true(),
            cors_origins: vec!["*".to_string()],
            request_timeout_secs: default_request_timeout(),
        }
    }
}

impl std::fmt::Display for AcpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

/// Tracing and observability configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TracingConfig {
    /// Enable tracing
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Jaeger endpoint for distributed tracing
    #[serde(default)]
    pub jaeger_endpoint: Option<String>,

    /// Enable JSON logging
    #[serde(default = "default_false")]
    pub json_logging: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            log_level: default_log_level(),
            jaeger_endpoint: None,
            json_logging: default_false(),
        }
    }
}

/// Quality evaluation configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QualityConfig {
    /// Default quality strategy for all agents
    #[serde(default)]
    pub default_strategy: Option<QualityStrategy>,

    /// Minimum acceptable quality score (0.0 - 1.0)
    #[serde(default = "default_min_quality_score")]
    pub min_quality_score: f32,

    /// Enable feedback loop
    #[serde(default = "default_true")]
    pub enable_feedback_loop: bool,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            default_strategy: None,
            min_quality_score: default_min_quality_score(),
            enable_feedback_loop: default_true(),
        }
    }
}


impl SystemConfig {
    pub fn new(indexing: IndexingConfig, rag: RagConfig,agent_network: AgentNetworkConfig,  storage: StorageConfig, embedding: EmbeddingConfig) -> Self {
        Self { indexing, rag,agent_network, storage, embedding}
    }

    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)
            .context(format!("Failed to read config file: {}", path))?;

        let config: SystemConfig = toml::from_str(&contents)
            .context("Failed to parse config file")?;

        Ok(config)
    }
    // Load configuration from file
    pub fn load_config(path: &str) -> anyhow::Result<SystemConfig> {
        SystemConfig::from_file(path)
    }

    /// Validate configuration values
    pub fn validate(&self) -> anyhow::Result<()> {
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


        self.agent_network.validate()?;

        for agent in self.agent_network.agents.clone(){
            agent.validate()?
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

// ============== Configuration Defaults ==============

fn default_temperature() -> f32 {
    0.7
}

fn default_max_tokens() -> usize {
    4096
}

fn default_context_window() -> usize {
    8192
}

fn default_false() -> bool {
    false
}

fn default_sample_rate() -> f32 {
    0.1
}

fn default_hitl_timeout() -> u64 {
    300
}

fn default_max_attempts() -> usize {
    3
}

fn default_backoff_ms() -> u64 {
    1000
}

fn default_backoff_multiplier() -> f32 {
    2.0
}

fn default_max_backoff_ms() -> u64 {
    60000
}

fn default_max_tokens_per_agent() -> usize {
    8192
}

fn default_pruning_threshold() -> f32 {
    0.8
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_request_timeout() -> u64 {
    30
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_min_quality_score() -> f32 {
    0.7
}


fn default_risk_threshold() -> RiskLevel {
    RiskLevel::High
}
