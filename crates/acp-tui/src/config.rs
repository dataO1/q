//! Configuration management for ACP TUI client

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// ACP server URL
    pub server_url: String,
    
    /// WebSocket configuration
    pub websocket: WebSocketConfig,
    
    /// UI configuration
    pub ui: UiConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// WebSocket connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// Connection timeout in seconds
    pub connect_timeout_secs: u64,
    
    /// Reconnect attempts
    pub max_reconnect_attempts: usize,
    
    /// Reconnect delay in seconds
    pub reconnect_delay_secs: u64,
    
    /// Ping interval in seconds
    pub ping_interval_secs: u64,
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Maximum history entries to keep
    pub max_history_entries: usize,
    
    /// Update interval for animations in milliseconds
    pub animation_interval_ms: u64,
    
    /// DAG visualization settings
    pub dag: DagConfig,
    
    /// Color theme
    pub theme: ThemeConfig,
}

/// DAG visualization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagConfig {
    /// Maximum nodes to display in DAG view
    pub max_visible_nodes: usize,
    
    /// Show node labels
    pub show_node_labels: bool,
    
    /// Compact mode for small terminals
    pub compact_mode: bool,
    
    /// Animation speed for wave transitions
    pub wave_transition_ms: u64,
}

/// Color theme configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Use colors (disable for monochrome terminals)
    pub use_colors: bool,
    
    /// Primary accent color
    pub primary_color: String,
    
    /// Success color
    pub success_color: String,
    
    /// Error color
    pub error_color: String,
    
    /// Warning color
    pub warning_color: String,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    
    /// Enable file logging
    pub log_to_file: bool,
    
    /// Log file path (if enabled)
    pub log_file: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:9999".to_string(),
            websocket: WebSocketConfig::default(),
            ui: UiConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            connect_timeout_secs: 10,
            max_reconnect_attempts: 5,
            reconnect_delay_secs: 2,
            ping_interval_secs: 30,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            max_history_entries: 1000,
            animation_interval_ms: 200,
            dag: DagConfig::default(),
            theme: ThemeConfig::default(),
        }
    }
}

impl Default for DagConfig {
    fn default() -> Self {
        Self {
            max_visible_nodes: 20,
            show_node_labels: true,
            compact_mode: false,
            wave_transition_ms: 500,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            use_colors: true,
            primary_color: "cyan".to_string(),
            success_color: "green".to_string(),
            error_color: "red".to_string(),
            warning_color: "yellow".to_string(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            log_to_file: false,
            log_file: None,
        }
    }
}

impl Config {
    /// Load configuration from file or use defaults
    pub fn load(
        config_path: Option<&String>,
        server_url: &str,
        log_level: &str,
    ) -> Result<Self> {
        let mut config = if let Some(path) = config_path {
            Self::from_file(path)?
        } else {
            Self::default()
        };
        
        // Override with command line arguments
        config.server_url = server_url.to_string();
        config.logging.level = log_level.to_string();
        
        Ok(config)
    }
    
    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path))?;
        
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path))?;
        
        Ok(config)
    }
    
    /// Save configuration to TOML file
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path))?;
        
        Ok(())
    }
    
    /// Generate example configuration file
    pub fn generate_example() -> String {
        let config = Self::default();
        toml::to_string_pretty(&config).unwrap_or_else(|_| "# Failed to generate config".to_string())
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate server URL
        url::Url::parse(&self.server_url)
            .with_context(|| format!("Invalid server URL: {}", self.server_url))?;
        
        // Validate timeouts
        if self.websocket.connect_timeout_secs == 0 {
            anyhow::bail!("WebSocket connect timeout must be greater than 0");
        }
        
        if self.websocket.reconnect_delay_secs == 0 {
            anyhow::bail!("WebSocket reconnect delay must be greater than 0");
        }
        
        // Validate UI settings
        if self.ui.max_history_entries == 0 {
            anyhow::bail!("Max history entries must be greater than 0");
        }
        
        if self.ui.dag.max_visible_nodes == 0 {
            anyhow::bail!("Max visible nodes must be greater than 0");
        }
        
        Ok(())
    }
}