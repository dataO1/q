//! ACP API client module
//!
//! Provides a wrapper around the generated OpenAPI client with additional
//! functionality for the TUI application.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// Include the generated API client
include!(concat!(env!("OUT_DIR"), "/acp_client.rs"));

/// High-level ACP client wrapper
pub struct AcpClient {
    inner: Client,
    base_url: String,
}

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub message: Option<String>,
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// Execute request
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub query: String,
    pub project_scope: Option<ProjectScope>,
}

/// Project scope information  
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectScope {
    pub root: String,
    pub current_file: Option<std::path::PathBuf>,
    pub language_distribution: Vec<(String, f32)>,
}

/// Execute response
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteResponse {
    pub conversation_id: String,
    pub message: Option<String>,
}

/// Agent capability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapability {
    pub agent_type: String,
    pub description: String,
    pub tools: Vec<String>,
}

/// Capabilities response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    pub agents: Vec<AgentCapability>,
    pub features: Vec<String>,
    pub version: String,
}

impl AcpClient {
    /// Create a new ACP client
    pub fn new(base_url: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        
        let inner = Client::new_with_client(&base_url, client);
        
        Ok(Self {
            inner,
            base_url: base_url.to_string(),
        })
    }
    
    /// Test connection to the ACP server
    pub async fn health_check(&self) -> Result<HealthResponse> {
        // Use direct reqwest since generated client might not have health endpoint
        let url = format!("{}/health", self.base_url);
        let response = reqwest::get(&url)
            .await
            .context("Failed to connect to ACP server")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Health check failed: HTTP {}", response.status());
        }
        
        let health = response
            .json::<HealthResponse>()
            .await
            .context("Failed to parse health response")?;
        
        Ok(health)
    }
    
    /// Get server capabilities
    pub async fn get_capabilities(&self) -> Result<CapabilitiesResponse> {
        let url = format!("{}/capabilities", self.base_url);
        let response = reqwest::get(&url)
            .await
            .context("Failed to get capabilities")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Get capabilities failed: HTTP {}", response.status());
        }
        
        let capabilities = response
            .json::<CapabilitiesResponse>()
            .await
            .context("Failed to parse capabilities response")?;
        
        Ok(capabilities)
    }
    
    /// Execute a query
    pub async fn execute_query(&self, request: ExecuteRequest) -> Result<ExecuteResponse> {
        let url = format!("{}/execute", self.base_url);
        let client = reqwest::Client::new();
        
        let response = client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to execute query")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Execute query failed: HTTP {} - {}", status, error_text);
        }
        
        let execute_response = response
            .json::<ExecuteResponse>()
            .await
            .context("Failed to parse execute response")?;
        
        Ok(execute_response)
    }
    
    /// Get WebSocket URL for streaming
    pub fn get_websocket_url(&self, conversation_id: &str) -> String {
        let ws_url = self.base_url.replace("http://", "ws://").replace("https://", "wss://");
        format!("{}/stream/{}", ws_url, conversation_id)
    }
    
    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Test connection to ACP server
pub async fn test_connection(server_url: &str) -> Result<HealthResponse> {
    let client = AcpClient::new(server_url)?;
    client.health_check().await
}

/// Detect project scope from current directory
pub fn detect_project_scope() -> Result<ProjectScope> {
    let current_dir = std::env::current_dir()
        .context("Failed to get current directory")?;
    
    let root = current_dir.to_string_lossy().to_string();
    
    // Simple language detection based on file extensions
    let language_distribution = detect_languages(&current_dir)?;
    
    Ok(ProjectScope {
        root,
        current_file: None,
        language_distribution,
    })
}

fn detect_languages(dir: &std::path::Path) -> Result<Vec<(String, f32)>> {
    use std::collections::HashMap;
    
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    let mut total_files = 0;
    
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    total_files += 1;
                    
                    if let Some(ext) = entry.path().extension() {
                        if let Some(ext_str) = ext.to_str() {
                            let language = extension_to_language(ext_str);
                            *file_counts.entry(language.to_string()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }
    
    if total_files == 0 {
        return Ok(vec![("Unknown".to_string(), 1.0)]);
    }
    
    let mut languages: Vec<(String, f32)> = file_counts
        .into_iter()
        .map(|(lang, count)| (lang, count as f32 / total_files as f32))
        .collect();
    
    languages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    Ok(languages)
}

fn extension_to_language(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "rs" => "Rust",
        "py" => "Python", 
        "js" => "JavaScript",
        "ts" => "TypeScript",
        "java" => "Java",
        "c" => "C",
        "cpp" | "cc" | "cxx" => "Cpp",
        "go" => "Go",
        "hs" => "Haskell",
        "lua" => "Lua",
        "yml" | "yaml" => "YAML",
        "sh" | "bash" => "Bash",
        "html" | "htm" => "HTML",
        "json" => "JSON",
        "rb" => "Ruby",
        "md" | "markdown" => "Markdown",
        "toml" => "TOML",
        "xml" => "XML",
        _ => "Unknown",
    }
}