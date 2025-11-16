//! Filesystem tool for file operations using tokio::fs
//!
//! Provides async file reading, writing, listing, and directory operations.

use crate::agents::ToolResult;
use crate::error::AgentNetworkResult;
use crate::tools::Tool;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::debug;

/// Filesystem tool for file operations
pub struct FilesystemTool {
    base_path: PathBuf,
}

impl FilesystemTool {
    /// Create new filesystem tool
    pub fn new(base_path: PathBuf) -> Self {
        debug!("FilesystemTool initialized for: {}", base_path.display());
        Self { base_path }
    }

    /// Read file contents
    async fn read(&self, path: &str) -> AgentNetworkResult<ToolResult> {
        let full_path = self.base_path.join(path);

        match fs::read_to_string(&full_path).await {
            Ok(contents) => Ok(ToolResult::success(
                "fs_read".to_string(),
                format!(
                    "Read {} bytes from {}\n{}",
                    contents.len(),
                    path,
                    &contents[..contents.len().min(1000)]
                ),
            )),
            Err(e) => Ok(ToolResult::error("fs_read".to_string(), e.to_string())),
        }
    }

    /// Write file contents
    async fn write(&self, path: &str, content: &str) -> AgentNetworkResult<ToolResult> {
        let full_path = self.base_path.join(path);

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await.ok();
        }

        match fs::write(&full_path, content).await {
            Ok(_) => Ok(ToolResult::success(
                "fs_write".to_string(),
                format!("Wrote {} bytes to {}", content.len(), path),
            )),
            Err(e) => Ok(ToolResult::error("fs_write".to_string(), e.to_string())),
        }
    }

    /// List directory contents
    async fn list(&self, path: &str) -> AgentNetworkResult<ToolResult> {
        let full_path = self.base_path.join(path);

        match fs::read_dir(&full_path).await {
            Ok(mut entries) => {
                let mut output = String::new();

                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Some(name) = entry.file_name().to_str() {
                        let file_type = match entry.file_type().await {
                            Ok(ft) if ft.is_dir() => "[DIR]",
                            Ok(ft) if ft.is_symlink() => "[LINK]",
                            _ => "[FILE]",
                        };

                        output.push_str(&format!("{} {}\n", file_type, name));
                    }
                }

                Ok(ToolResult::success("fs_list".to_string(), output))
            }
            Err(e) => Ok(ToolResult::error("fs_list".to_string(), e.to_string())),
        }
    }

    /// Create directory
    async fn mkdir(&self, path: &str) -> AgentNetworkResult<ToolResult> {
        let full_path = self.base_path.join(path);

        match fs::create_dir_all(&full_path).await {
            Ok(_) => Ok(ToolResult::success(
                "fs_mkdir".to_string(),
                format!("Created directory: {}", path),
            )),
            Err(e) => Ok(ToolResult::error("fs_mkdir".to_string(), e.to_string())),
        }
    }

    /// Delete file
    async fn delete(&self, path: &str) -> AgentNetworkResult<ToolResult> {
        let full_path = self.base_path.join(path);

        match fs::remove_file(&full_path).await {
            Ok(_) => Ok(ToolResult::success(
                "fs_delete".to_string(),
                format!("Deleted: {}", path),
            )),
            Err(e) => Ok(ToolResult::error("fs_delete".to_string(), e.to_string())),
        }
    }

    /// Check if file exists
    async fn exists(&self, path: &str) -> AgentNetworkResult<ToolResult> {
        let full_path = self.base_path.join(path);
        let exists = fs::metadata(&full_path).await.is_ok();

        Ok(ToolResult::success(
            "fs_exists".to_string(),
            format!("File exists: {}", exists),
        ))
    }

    /// Get file metadata
    async fn metadata(&self, path: &str) -> AgentNetworkResult<ToolResult> {
        let full_path = self.base_path.join(path);

        match fs::metadata(&full_path).await {
            Ok(meta) => {
                let output = format!(
                    "Size: {} bytes\nDirectory: {}\nReadonly: {}",
                    meta.len(),
                    meta.is_dir(),
                    meta.permissions().readonly()
                );

                Ok(ToolResult::success("fs_metadata".to_string(), output))
            }
            Err(e) => Ok(ToolResult::error("fs_metadata".to_string(), e.to_string())),
        }
    }
}

#[async_trait]
impl Tool for FilesystemTool {
    async fn execute(
        &self,
        command: &str,
        params: HashMap<String, String>,
    ) -> AgentNetworkResult<ToolResult> {
        match command {
            "read" => {
                let path = params.get("path").ok_or_else(|| {
                    crate::error::AgentNetworkError::Tool("read requires 'path'".to_string())
                })?;
                self.read(path).await
            }
            "write" => {
                let path = params.get("path").ok_or_else(|| {
                    crate::error::AgentNetworkError::Tool("write requires 'path'".to_string())
                })?;
                let content = params.get("content").ok_or_else(|| {
                    crate::error::AgentNetworkError::Tool("write requires 'content'".to_string())
                })?;
                self.write(path, content).await
            }
            "list" => {
                let path = params.get("path").cloned().unwrap_or_else(|| ".".to_string());
                self.list(&path).await
            }
            "mkdir" => {
                let path = params.get("path").ok_or_else(|| {
                    crate::error::AgentNetworkError::Tool("mkdir requires 'path'".to_string())
                })?;
                self.mkdir(path).await
            }
            "delete" => {
                let path = params.get("path").ok_or_else(|| {
                    crate::error::AgentNetworkError::Tool("delete requires 'path'".to_string())
                })?;
                self.delete(path).await
            }
            "exists" => {
                let path = params.get("path").ok_or_else(|| {
                    crate::error::AgentNetworkError::Tool("exists requires 'path'".to_string())
                })?;
                self.exists(path).await
            }
            "metadata" => {
                let path = params.get("path").ok_or_else(|| {
                    crate::error::AgentNetworkError::Tool("metadata requires 'path'".to_string())
                })?;
                self.metadata(path).await
            }
            _ => Err(crate::error::AgentNetworkError::Tool(format!(
                "Unknown filesystem command: {}",
                command
            ))),
        }
    }

    fn name(&self) -> &str {
        "filesystem"
    }

    fn description(&self) -> &str {
        "Filesystem operations: read, write, list, mkdir, delete, exists, metadata"
    }

    fn available_commands(&self) -> Vec<String> {
        vec![
            "read".to_string(),
            "write".to_string(),
            "list".to_string(),
            "mkdir".to_string(),
            "delete".to_string(),
            "exists".to_string(),
            "metadata".to_string(),
        ]
    }

    fn validate_params(&self, command: &str, params: &HashMap<String, String>) -> AgentNetworkResult<()> {
        match command {
            "read" | "delete" | "exists" | "metadata" => {
                if !params.contains_key("path") {
                    return Err(crate::error::AgentNetworkError::Tool(
                        format!("{} requires 'path' parameter", command),
                    ));
                }
                Ok(())
            }
            "write" => {
                if !params.contains_key("path") || !params.contains_key("content") {
                    return Err(crate::error::AgentNetworkError::Tool(
                        "write requires 'path' and 'content' parameters".to_string(),
                    ));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_tool_creation() {
        let tool = FilesystemTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.name(), "filesystem");
        assert!(!tool.available_commands().is_empty());
    }

    #[test]
    fn test_available_commands() {
        let tool = FilesystemTool::new(PathBuf::from("/tmp"));
        let commands = tool.available_commands();
        assert!(commands.contains(&"read".to_string()));
        assert!(commands.contains(&"write".to_string()));
    }
}
