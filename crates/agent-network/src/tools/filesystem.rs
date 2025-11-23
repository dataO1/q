//! Filesystem tool for file operations using tokio::fs

use crate::{error::AgentNetworkResult, tools::ToolExecutor};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use serde_json::{Value, json};
use anyhow::Result;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, instrument};
use anyhow::anyhow;

#[derive(Debug)]
pub struct FilesystemTool {
    base_path: PathBuf,
}

impl FilesystemTool {
    pub fn new(base_path: &str) -> Self {
        let base_path = PathBuf::from(base_path);
        debug!("FilesystemTool initialized for: {}", base_path.display());
        Self { base_path }
    }

    // Helper to resolve full path safely
    fn is_path_allowed(&self, path: &Path) -> bool {
        path.starts_with(self.base_path.clone())
    }
}

#[async_trait]
impl ToolExecutor for FilesystemTool {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    fn description(&self) -> String {
        format!(r#"Filesystem tool for managing files and directories. All paths must be absolute and must be valid subpaths of the working directory: {}

        COMMANDS:

        read
        - Reads and returns the entire content of a file
        - Required: path (string) - absolute file path
        - Returns: file contents as text
        - Example: {{"command": "read", "path": "{}/src/main.rs"}}

        write
        - Writes content to a file, creating parent directories if needed
        - Required: path (string), content (string)
        - Returns: confirmation with byte count
        - Example: {{"command": "write", "path": "{}/output/result.txt", "content": "Hello World"}}

        list
        - Lists all files and directories in a directory
        - Required: path (string) - directory path
        - Returns: list with [DIR] or [FILE] prefix for each entry
        - Example: {{"command": "list", "path": "{}/src/"}}

        mkdir
        - Creates a directory and all parent directories
        - Required: path (string)
        - Returns: confirmation message
        - Example: {{"command": "mkdir", "path": "{}/output/logs"}}

        delete
        - Deletes a file (not directories)
        - Required: path (string)
        - Returns: confirmation message
        - Example: {{"command": "delete", "path": "{}/temp/old.txt"}}

        exists
        - Checks if a file or directory exists
        - Required: path (string)
        - Returns: "Exists: true" or "Exists: false"
        - Example: {{"command": "exists", "path": "{}/config.toml"}}

        metadata
        - Returns file/directory information
        - Required: path (string)
        - Returns: size, directory status, readonly status
        - Example: {{"command": "metadata", "path": "{}/Cargo.toml"}}

        IMPORTANT:
        - All paths must be absolute and must be valid subpaths of the working directory: {}
        - Use forward slashes in paths
        - Parent directories are auto-created for write operations
        - Working directory is: {}
        "#, 
        self.base_path.display(),
        self.base_path.display(),
        self.base_path.display(),
        self.base_path.display(),
        self.base_path.display(),
        self.base_path.display(),
        self.base_path.display(),
        self.base_path.display(),
        self.base_path.display(),
        self.base_path.display())
    }

    fn provide_tool_info(&self) -> ollama_rs::generation::tools::ToolInfo {
        // Provide the tool schema info for the LLM prompt
        // For brevity, use a simple generic JSON schema here or implement schemars::JsonSchema for Params struct
        let parameters = json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "enum": ["read", "write", "list", "mkdir", "delete", "exists", "metadata"]
                },
                "path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["command", "path"]
        });

        ollama_rs::generation::tools::ToolInfo {
            tool_type: ollama_rs::generation::tools::ToolType::Function,
            function: ollama_rs::generation::tools::ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description(),
                parameters: serde_json::from_value(parameters).unwrap(), // converts to schemars::schema::Schema
            },
        }
    }

    #[instrument(skip(self, parameters), fields(tool = "filesystem"))]
    async fn call(&self, parameters: Value) -> Result<String> {
        // Extract command and params from JSON
        let command = parameters.get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'command' parameter"))?;

        let path = parameters.get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'path' parameter"))?;
        let path = Path::new(path);

        if !self.is_path_allowed(path) {
            Err(anyhow!("Given path is not a subpath of the working dir!"))
        }else{

            match command {
                "read" => {
                    let contents = fs::read_to_string(path).await?;
                    Ok(contents)
                }
                "write" => {
                    let content = parameters.get("content")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'content' parameter for write"))?;
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).await?;
                    }
                    fs::write(path, content).await?;
                    Ok(format!("Wrote {} bytes", content.len()))
                }
                "list" => {
                    let mut entries = fs::read_dir(path).await?;
                    let mut listing = Vec::new();
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if let Ok(file_type) = entry.file_type().await {
                            let typ = if file_type.is_dir() { "[DIR]" } else { "[FILE]" };
                            if let Some(name) = entry.file_name().to_str() {
                                listing.push(format!("{} {}", typ, name));
                            }
                        }
                    }
                    Ok(listing.join("\n"))
                }
                "mkdir" => {
                    fs::create_dir_all(path).await?;
                    Ok(format!("Created directory: {:?}", path))
                }
                "delete" => {
                    fs::remove_file(path).await?;
                    Ok(format!("Deleted file: {:?}", path))
                }
                "exists" => {
                    let exists = fs::metadata(path).await.is_ok();
                    Ok(format!("Exists: {}", exists))
                }
                "metadata" => {
                    let metadata = fs::metadata(path).await?;
                    Ok(format!(
                        "Size: {} bytes\nIs Directory: {}\nReadonly: {}",
                        metadata.len(),
                        metadata.is_dir(),
                        metadata.permissions().readonly()
                    ))
                }
                _ => Err(anyhow::anyhow!("Unknown filesystem command: {}", command)),
            }
        }
    }
}
