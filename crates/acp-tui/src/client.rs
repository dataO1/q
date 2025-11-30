//! ACP API client module
//!
//! Minimal wrapper around the generated OpenAPI client, using only generated types and methods.

use anyhow::{Context, Result};
use std::time::Duration;
use tracing::{debug, instrument};

// Include the generated API client
include!(concat!(env!("OUT_DIR"), "/acp_client.rs"));

// Re-export the generated types for convenience
pub use types::*;

/// Minimal ACP client wrapper that uses only generated OpenAPI client methods
#[derive(Clone)]
pub struct AcpClient {
    /// Generated OpenAPI client
    inner: Client,
    /// Base URL for WebSocket URL generation
    base_url: String,
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
    
    /// Get direct access to the generated client for all API operations
    pub fn client(&self) -> &Client {
        &self.inner
    }
    
    /// Get WebSocket URL for streaming with subscription_id
    pub fn get_websocket_url(&self, subscription_id: &str) -> String {
        let ws_url = self.base_url.replace("http://", "ws://").replace("https://", "wss://");
        format!("{}/stream/{}", ws_url, subscription_id)
    }
    
    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Test connection to ACP server using generated client
#[instrument(fields(server_url = %server_url))]
pub async fn test_connection(server_url: &str) -> Result<HealthResponse> {
    let client = AcpClient::new(server_url)
        .context("Failed to create ACP client")?;
    
    let response = client.client().health_check()
        .await
        .context("Health check failed")?;
    
    Ok(response.into_inner())
}

/// Detect project scope from current directory using generated API types
pub fn detect_project_scope() -> Result<types::ProjectScope> {
    let current_dir = std::env::current_dir()
        .context("Failed to get current directory")?;
    
    let root = current_dir.to_string_lossy().to_string();
    
    // Simple language detection based on file extensions
    let language_distribution = detect_languages(&current_dir)?;
    
    Ok(types::ProjectScope {
        root,
        current_file: None, // Generated type expects Option<String>, not Option<PathBuf>
        language_distribution,
    })
}

fn detect_languages(dir: &std::path::Path) -> Result<std::collections::HashMap<String, f32>> {
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
        let mut result = HashMap::new();
        result.insert("Unknown".to_string(), 1.0);
        return Ok(result);
    }
    
    let languages: HashMap<String, f32> = file_counts
        .into_iter()
        .map(|(lang, count)| (lang, count as f32 / total_files as f32))
        .collect();
    
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