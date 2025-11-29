//! ACP API client module
//!
//! Provides a wrapper around the generated OpenAPI client with additional
//! functionality for the TUI application.

use anyhow::{anyhow, Context, Result};
use std::time::Duration;
use tracing::{error, instrument, warn};

// Include the generated API client
include!(concat!(env!("OUT_DIR"), "/acp_client.rs"));

// Re-export the generated types for convenience
pub use types::*;

/// High-level ACP client wrapper
#[derive(Clone)]
pub struct AcpClient {
    inner: Client,
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
    
    /// Test connection to the ACP server
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<HealthResponse> {
        let response = self.inner.health_check()
            .await
            .map_err(|e| {
                error!("Health check failed: {}", e);
                match e.status() {
                    Some(status) if status.is_server_error() => {
                        anyhow!("Server error during health check: {} ({})", e, status)
                    },
                    Some(status) if status.is_client_error() => {
                        anyhow!("Client error during health check: {} ({})", e, status)
                    },
                    Some(status) => {
                        anyhow!("Unexpected response during health check: {} ({})", e, status)
                    },
                    None => {
                        anyhow!("Network error during health check: {}", e)
                    }
                }
            })
            .context("Failed to connect to ACP server")?;
        
        Ok(response.into_inner())
    }
    
    /// Get server capabilities
    #[instrument(skip(self))]
    pub async fn get_capabilities(&self) -> Result<CapabilitiesResponse> {
        let response = self.inner.list_capabilities()
            .await
            .map_err(|e| {
                error!("Capabilities request failed: {}", e);
                match e.status() {
                    Some(status) if status.is_server_error() => {
                        anyhow!("Server error getting capabilities: {} ({})", e, status)
                    },
                    Some(status) if status.is_client_error() => {
                        anyhow!("Client error getting capabilities: {} ({})", e, status)
                    },
                    Some(status) => {
                        anyhow!("Unexpected response getting capabilities: {} ({})", e, status)
                    },
                    None => {
                        anyhow!("Network error getting capabilities: {}", e)
                    }
                }
            })
            .context("Failed to get server capabilities")?;
        
        Ok(response.into_inner())
    }
    
    /// Execute a query
    #[instrument(skip(self, project_scope), fields(query = %query))]
    pub async fn query(&self, query: &str, project_scope: ProjectScope) -> Result<QueryResponse> {
        let request = QueryRequest {
            query: query.to_string(),
            project_scope,
            conversation_id: None,
        };
        
        let response = self.inner.query_task(&request)
            .await
            .map_err(|e| {
                error!("Query execution failed: {}", e);
                match e.status() {
                    Some(status) if status.as_u16() == 400 => {
                        anyhow!("Invalid query request: Check your query syntax and project context")
                    },
                    Some(status) if status.as_u16() == 401 => {
                        anyhow!("Authentication failed: Check your API credentials")
                    },
                    Some(status) if status.as_u16() == 403 => {
                        anyhow!("Permission denied: You don't have access to execute queries")
                    },
                    Some(status) if status.as_u16() == 404 => {
                        anyhow!("API endpoint not found: Check your server URL")
                    },
                    Some(status) if status.as_u16() == 422 => {
                        // Try to extract the actual validation error message from response body
                        match extract_error_body(&e) {
                            Ok(body) if !body.is_empty() => {
                                anyhow!("Validation error: {}", body)
                            },
                            _ => {
                                anyhow!("Validation error: Request data is invalid or incomplete")
                            }
                        }
                    },
                    Some(status) if status.as_u16() == 500 => {
                        anyhow!("Server error: The agent network encountered an internal error")
                    },
                    Some(status) if status.as_u16() == 503 => {
                        anyhow!("Service unavailable: Agent network is temporarily down")
                    },
                    Some(status) if status.is_server_error() => {
                        anyhow!("Server error during query execution: {} ({})", e, status)
                    },
                    Some(status) if status.is_client_error() => {
                        anyhow!("Client error during query execution: {} ({})", e, status)
                    },
                    Some(status) => {
                        anyhow!("Unexpected response during query execution: {} ({})", e, status)
                    },
                    None => {
                        anyhow!("Network error during query execution: {}", e)
                    }
                }
            })
            .context("Failed to execute query")?;
        
        Ok(response.into_inner())
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
#[instrument(fields(server_url = %server_url))]
pub async fn test_connection(server_url: &str) -> Result<HealthResponse> {
    let client = AcpClient::new(server_url)
        .context("Failed to create ACP client")?;
    
    client.health_check()
        .await
        .context("Health check failed")
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

/// Extract error response body text from progenitor Error
/// 
/// Attempts to extract the actual error message from the HTTP response body.
/// This is particularly useful for validation errors (422) that contain specific
/// field-level error descriptions.
fn extract_error_body(error: &progenitor_client::Error<types::ErrorResponse>) -> Result<String> {
    // Convert the error to a string and try to parse useful information
    let error_str = error.to_string();
    
    // For now, extract what we can from the error string
    // The progenitor error typically contains response details
    if let Some(start) = error_str.find("Response { url:") {
        // Try to extract any meaningful error text from the error string
        if error_str.contains("422") {
            // For 422 errors, the server sends plain text error messages
            // We can't easily get the body from progenitor::Error directly,
            // but we can at least indicate it's a validation error
            return Ok("Invalid or incomplete request data - check required fields".to_string());
        }
    }
    
    // Try to parse JSON error response if present
    if error_str.contains("ErrorResponse") {
        // This suggests the server returned a JSON ErrorResponse
        return Ok("Server returned a structured error response".to_string());
    }
    
    // Fallback: try to extract any useful information from the error string
    if let Some(msg) = extract_meaningful_error_text(&error_str) {
        Ok(msg)
    } else {
        Ok("Error details not available".to_string())
    }
}

/// Extract meaningful error text from a complex error string
fn extract_meaningful_error_text(error_str: &str) -> Option<String> {
    // Look for common error patterns and extract useful information
    
    // Check for validation error patterns
    if error_str.contains("missing field") {
        if let Some(start) = error_str.find("missing field") {
            if let Some(end) = error_str[start..].find("at line") {
                return Some(error_str[start..start + end].trim().to_string());
            }
        }
    }
    
    // Check for deserialization errors
    if error_str.contains("Failed to deserialize") {
        if let Some(start) = error_str.find("Failed to deserialize") {
            if let Some(end) = error_str[start..].find("at line") {
                return Some(error_str[start..start + end].trim().to_string());
            }
        }
    }
    
    // Check for other common validation patterns
    if error_str.contains("invalid") || error_str.contains("required") {
        // Try to extract a meaningful snippet
        let words: Vec<&str> = error_str.split_whitespace().collect();
        for window in words.windows(8) {
            let phrase = window.join(" ");
            if phrase.contains("invalid") || phrase.contains("required") || phrase.contains("missing") {
                return Some(phrase);
            }
        }
    }
    
    None
}