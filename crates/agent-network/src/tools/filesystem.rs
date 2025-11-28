//! Individual filesystem tools for file operations using tokio::fs

use async_trait::async_trait;
use schemars::JsonSchema;
use std::path::{Path, PathBuf};
use tokio::fs;
use serde_json::{Value, json};
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, warn, instrument};
use anyhow::anyhow;
use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};
use crate::tools::{Tool,ToolResult, TypedTool};
use serde::{Deserialize, Serialize};

pub const FILESYSTEM_PREAMBLE: &str = r#"

## FILESYSTEM WORKSPACE RULES
You are operating within a restricted project workspace.
1. **Relative Paths Only**: All file paths must be relative to the project root (e.g., use `src/main.rs`, not `/home/user/src/main.rs`).
2. **Workspace Confinement**: You cannot access or modify files outside this workspace.
3. **File Creation**: If a target directory does not exist, you must create it first or assume the tool handles it (check tool descriptions).

### CRITICAL: When passing code content in JSON, do NOT double-escape newlines. Use standard JSON string escaping (e.g. use \n for a newline, not \\n).
"#;

// Shared base functionality for all filesystem tools
#[derive(Debug, Clone)]
struct FilesystemBase {
    base_path: PathBuf,
}

impl FilesystemBase {
    fn new(base_path: &str) -> Self {
        let base_path = PathBuf::from(base_path);
        let base_path = std::fs::canonicalize(&base_path)
            .unwrap_or_else(|_| PathBuf::from(base_path));
        Self { base_path }
    }

    fn resolve_secure_path(&self, relative_path: &str) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let relative_path = relative_path.trim_start_matches('/');
        let target_path = self.base_path.join(relative_path);

        let canonical_target = if target_path.exists() {
            std::fs::canonicalize(&target_path)?
        } else {
            // Find the first existing ancestor
            let mut ancestor = target_path.parent();
            while let Some(p) = ancestor {
                if p.exists() {
                    let canonical_ancestor = std::fs::canonicalize(p)?;

                    // Security check on the existing ancestor
                    if !canonical_ancestor.starts_with(&self.base_path) {
                        return Err("Access denied".into());
                    }

                    // Build the full path from the safe ancestor
                    let remainder = target_path.strip_prefix(p)
                        .map_err(|_| "Path resolution error")?;
                    return Ok(canonical_ancestor.join(remainder));
                }
                ancestor = p.parent();
            }

            // If no ancestor exists (shouldn't happen if base_path is valid), fall back
            return Err("No valid parent directory found".into());
        };

        // Final security check
        if !canonical_target.starts_with(&self.base_path) {
            return Err(format!("Access denied: {} is outside the workspace", relative_path).into());
        }

        debug!("resolved secure path: {} -> {}", relative_path, canonical_target.display());
        Ok(canonical_target)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileMetaParam {
    #[schemars(description = "The path of the file from which metadata should be retrieved.")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileExistsParam {
    #[schemars(description = "The path of the file check if it exists.")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteFileParam {
    #[schemars(description = "The path of the file to delete.")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateDirParam {
    #[schemars(description = "The path of the path to create.")]
    pub path: String,
}


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadParam {
    #[schemars(description = "The path of the file to read.")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListDirParam {
    #[schemars(description = "The path of the directory to list files in.")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteParam {
    #[schemars(description = "The path of the file to write.")]
    pub path: String,
    #[schemars(description = "The content of the file to write")]
    pub content: String,
}

// ReadFileTool - Read file contents
#[derive(Debug, Clone)]
pub struct ReadFileTool {
    base: FilesystemBase,
}

impl ReadFileTool {
    pub fn new(base_path: &str) -> Self {
        Self {
            base: FilesystemBase::new(base_path),
        }
    }
}

#[async_trait]
impl TypedTool for ReadFileTool {
    type Params = ListDirParam;

    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Provide a relative path."
    }

    #[instrument(name = "read_file_tool", skip(self), fields(
        tool_name = "read_file",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        file_size = tracing::field::Empty,
        error = tracing::field::Empty
    ))]
    async fn call(
        &self,
        parameters: Self::Params
    ) -> Result<ToolResult> {
            let current_span = tracing::Span::current();
            current_span.record("path", parameters.path.as_str());

            let target_path = match self.base.resolve_secure_path(&parameters.path) {
                Ok(path) => path,
                Err(e) => {
                    let error_msg = format!("Path access error: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                        success: false,
                        output: format!("Error: {}", error_msg),
                    });
                }
            };

            let contents = match fs::read_to_string(&target_path).await {
                Ok(contents) => contents,
                Err(e) => {
                    let error_msg = format!("Error reading file: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                    success: false,
                    output: error_msg,
                });
                }
            };
            current_span.record("success", true);
            current_span.record("file_size", contents.len());

            Ok(ToolResult {
                success: true,
                output: contents,
            })
    }
}

// WriteFileTool - Write content to file
#[derive(Debug, Clone)]
pub struct WriteFileTool {
    base: FilesystemBase,
}

impl WriteFileTool {
    pub fn new(base_path: &str) -> Self {
        Self {
            base: FilesystemBase::new(base_path),
        }
    }
}

#[async_trait]
impl TypedTool for WriteFileTool {
    type Params = WriteParam;
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates parent directories if needed. Provide relative path and content."
    }

    #[instrument(name = "write_file_tool", skip(self), fields(
        tool_name = "write_file",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        content_size = tracing::field::Empty,
        error = tracing::field::Empty
    ))]
    async fn call(
        &self,
        parameters: Self::Params
    ) -> Result<ToolResult> {
            let current_span = tracing::Span::current();
            current_span.record("path", parameters.path.as_str());
            current_span.record("content_size", parameters.content.len());

            let target_path = match self.base.resolve_secure_path(&parameters.path) {
                Ok(path) => path,
                Err(e) => {
                    let error_msg = format!("Path access error: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                        success: false,
                        output: format!("Error: {}", error_msg),
                    });
                }
            };

            if let Some(parent) = target_path.parent() {
                if let Err(e) = fs::create_dir_all(parent).await {
                    let error_msg = format!("Error creating parent directories: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                    success: false,
                    output: error_msg,
                });
                }
            }

            if let Err(e) = fs::write(&target_path, &parameters.content).await {
                let error_msg = format!("Error writing file: {}", e);
                current_span.record("success", false);
                current_span.record("error", error_msg.as_str());
                return Ok(ToolResult {
                    success: false,
                    output: error_msg,
                });
            };
            current_span.record("success", true);

            Ok(ToolResult {
                success: true,
                output: format!("Wrote {} bytes to {}", parameters.content.len(), target_path.display()),
            })
    }
}

// ListDirectoryTool - List directory contents
#[derive(Debug, Clone)]
pub struct ListDirectoryTool {
    base: FilesystemBase,
}

impl ListDirectoryTool {
    pub fn new(base_path: &str) -> Self {
        Self {
            base: FilesystemBase::new(base_path),
        }
    }
}

#[async_trait]
impl TypedTool for ListDirectoryTool {
    type Params = ListDirParam;

    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List the contents of a directory. Provide a relative path to a directory."
    }

    #[instrument(name = "list_directory_tool", skip(self), fields(
        tool_name = "list_directory",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        entry_count = tracing::field::Empty,
        error = tracing::field::Empty
    ))]
    async fn call(
        &self,
        parameters: Self::Params
    ) -> Result<ToolResult> {
            let current_span = tracing::Span::current();
            current_span.record("path", parameters.path.as_str());

            let target_path = match self.base.resolve_secure_path(&parameters.path) {
                Ok(path) => path,
                Err(e) => {
                    let error_msg = format!("Path access error: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                        success: false,
                        output: format!("Error: {}", error_msg),
                    });
                }
            };

            let mut entries = match fs::read_dir(&target_path).await {
                Ok(entries) => entries,
                Err(e) => {
                    let error_msg = format!("Error reading directory: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                    success: false,
                    output: error_msg,
                });
                }
            };
            let mut listing = Vec::new();

            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(file_type) = entry.file_type().await {
                    let typ = if file_type.is_dir() { "[DIR]" } else { "[FILE]" };
                    if let Some(name) = entry.file_name().to_str() {
                        listing.push(format!("{} {}", typ, name));
                    }
                }
            }

            current_span.record("success", true);
            current_span.record("entry_count", listing.len());

            Ok(ToolResult {
                success: true,
                output: listing.join("\n"),
            })
    }
}

// CreateDirectoryTool - Create directories
#[derive(Debug, Clone)]
pub struct CreateDirectoryTool {
    base: FilesystemBase,
}

impl CreateDirectoryTool {
    pub fn new(base_path: &str) -> Self {
        Self {
            base: FilesystemBase::new(base_path),
        }
    }
}

#[async_trait]
impl TypedTool for CreateDirectoryTool {
    type Params = CreateDirParam;

    fn name(&self) -> &str {
        "create_directory"
    }

    fn description(&self) -> &str {
        "Create a directory and all parent directories if they don't exist. Provide a relative path."
    }

    #[instrument(name = "create_directory_tool", skip(self), fields(
        tool_name = "create_directory",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        error = tracing::field::Empty
    ))]
    async fn call(
        &self,
        parameters: Self::Params
    ) -> Result<ToolResult> {
            let current_span = tracing::Span::current();
            current_span.record("path", parameters.path.as_str());

            let target_path = match self.base.resolve_secure_path(&parameters.path) {
                Ok(path) => path,
                Err(e) => {
                    let error_msg = format!("Path access error: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                        success: false,
                        output: format!("Error: {}", error_msg),
                    });
                }
            };

            if let Err(e) = fs::create_dir_all(&target_path).await {
                let error_msg = format!("Error creating directory: {}", e);
                current_span.record("success", false);
                current_span.record("error", error_msg.as_str());
                return Ok(ToolResult {
                    success: false,
                    output: error_msg,
                });
            };
            current_span.record("success", true);

            Ok(ToolResult {
                success: true,
                output: format!("Created directory: {}", target_path.display()),
            })
    }
}

// DeleteFileTool - Delete files
#[derive(Debug, Clone)]
pub struct DeleteFileTool {
    base: FilesystemBase,
}

impl DeleteFileTool {
    pub fn new(base_path: &str) -> Self {
        Self {
            base: FilesystemBase::new(base_path),
        }
    }
}

#[async_trait]
impl TypedTool for DeleteFileTool {
    type Params = DeleteFileParam;

    fn name(&self) -> &str {
        "delete_file"
    }

    fn description(&self) -> &str {
        "Delete a file. Provide a relative path to the file to delete."
    }

    #[instrument(name = "delete_file_tool", skip(self), fields(
        tool_name = "delete_file",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        error = tracing::field::Empty
    ))]
    async fn call(
        &self,
        parameters: Self::Params
    ) -> Result<ToolResult> {
            let current_span = tracing::Span::current();
            current_span.record("path", parameters.path.as_str());

            let target_path = match self.base.resolve_secure_path(&parameters.path) {
                Ok(path) => path,
                Err(e) => {
                    let error_msg = format!("Path access error: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                        success: false,
                        output: format!("Error: {}", error_msg),
                    });
                }
            };

            if let Err(e) = fs::remove_file(&target_path).await {
                let error_msg = format!("Error deleting file: {}", e);
                current_span.record("success", false);
                current_span.record("error", error_msg.as_str());
                return Ok(ToolResult {
                    success: false,
                    output: error_msg,
                });
            };
            current_span.record("success", true);

            Ok(ToolResult {
                success: true,
                output: format!("Deleted file: {}", target_path.display()),
            })
    }
}

// FileExistsTool - Check if file exists
#[derive(Debug, Clone)]
pub struct FileExistsTool {
    base: FilesystemBase,
}

impl FileExistsTool {
    pub fn new(base_path: &str) -> Self {
        Self {
            base: FilesystemBase::new(base_path),
        }
    }
}

#[async_trait]
impl TypedTool for FileExistsTool {
    type Params = FileExistsParam;

    fn name(&self) -> &str {
        "file_exists"
    }

    fn description(&self) -> &str {
        "Check if a file or directory exists. Provide a relative path."
    }

    #[instrument(name = "file_exists_tool", skip(self), fields(
        tool_name = "file_exists",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        exists = tracing::field::Empty,
        error = tracing::field::Empty
    ))]
    async fn call(
        &self,
        parameters: Self::Params
    ) -> Result<ToolResult> {
            let current_span = tracing::Span::current();
            current_span.record("path", parameters.path.as_str());

            let target_path = match self.base.resolve_secure_path(&parameters.path) {
                Ok(path) => path,
                Err(e) => {
                    let error_msg = format!("Path access error: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                        success: false,
                        output: format!("Error: {}", error_msg),
                    });
                }
            };

            let exists = fs::metadata(&target_path).await.is_ok();
            current_span.record("success", true);
            current_span.record("exists", exists);

            Ok(ToolResult {
                success: true,
                output: format!("Exists: {}", exists),
            })
    }
}

// FileMetadataTool - Get file metadata
#[derive(Debug, Clone)]
pub struct FileMetadataTool {
    base: FilesystemBase,
}

impl FileMetadataTool {
    pub fn new(base_path: &str) -> Self {
        Self {
            base: FilesystemBase::new(base_path),
        }
    }
}

#[async_trait]
impl TypedTool for FileMetadataTool {
    type Params = FileMetaParam;

    fn name(&self) -> &str {
        "file_metadata"
    }

    fn description(&self) -> &str  {
        "Get metadata information about a file or directory (size, type, permissions). Provide a relative path."
    }

    #[instrument(name = "file_metadata_tool", skip(self), fields(
        tool_name = "file_metadata",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        file_size = tracing::field::Empty,
        is_dir = tracing::field::Empty,
        error = tracing::field::Empty
    ))]
    async fn call(
        &self,
        parameters: Self::Params
    ) -> Result<ToolResult> {
            let current_span = tracing::Span::current();
            current_span.record("path", parameters.path.as_str());

            let target_path = match self.base.resolve_secure_path(&parameters.path) {
                Ok(path) => path,
                Err(e) => {
                    let error_msg = format!("Path access error: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                        success: false,
                        output: format!("Error: {}", error_msg),
                    });
                }
            };

            let metadata = match fs::metadata(&target_path).await {
                Ok(metadata) => metadata,
                Err(e) => {
                    let error_msg = format!("Error getting file metadata: {}", e);
                    current_span.record("success", false);
                    current_span.record("error", error_msg.as_str());
                    return Ok(ToolResult {
                    success: false,
                    output: error_msg,
                });
                }
            };
            current_span.record("success", true);
            current_span.record("file_size", metadata.len());
            current_span.record("is_dir", metadata.is_dir());

            Ok(ToolResult {
                success: true,
                output: format!(
                    "Size: {} bytes\nIs Directory: {}\nReadonly: {}",
                    metadata.len(),
                    metadata.is_dir(),
                    metadata.permissions().readonly()
                ),
            })
    }
}

