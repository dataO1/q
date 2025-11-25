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
use ollama_rs::generation::tools::Tool;
use serde::{Deserialize, Serialize};

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

    fn is_path_allowed(&self, path: &Path) -> bool {
        path.starts_with(self.base_path.clone())
    }
}

// Tool parameter structs
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathParam {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteParam {
    pub path: String,
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

impl Tool for ReadFileTool {
    type Params = PathParam;

    fn name() -> String {
        "read_file".to_string()
    }

    fn description() -> String {
        "Read the contents of a file. Provide an absolute path.".to_string()
    }

    #[instrument(name = "read_file_tool", skip(self), fields(
        tool_name = "read_file",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        file_size = tracing::field::Empty
    ))]
    fn call(
        &mut self,
        parameters: Self::Params
    ) -> impl std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send + Sync {
        async move {
            let current_span = tracing::Span::current();
            let path = Path::new(&parameters.path);

            current_span.record("path", path.display().to_string().as_str());

            if !self.base.is_path_allowed(path) {
                current_span.record("success", false);
                return Err(anyhow!("Path not allowed: {}", path.display()).into());
            }

            let contents = fs::read_to_string(path).await?;
            current_span.record("success", true);
            current_span.record("file_size", contents.len());

            Ok(contents)
        }
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

impl Tool for WriteFileTool {
    type Params = WriteParam;

    fn name() -> String {
        "write_file".to_string()
    }

    fn description() -> String {
        "Write content to a file. Creates parent directories if needed. Provide absolute path and content.".to_string()
    }

    #[instrument(name = "write_file_tool", skip(self), fields(
        tool_name = "write_file",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        content_size = tracing::field::Empty
    ))]
    fn call(
        &mut self,
        parameters: Self::Params
    ) -> impl std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send + Sync {
        async move {
            let current_span = tracing::Span::current();
            let path = Path::new(&parameters.path);

            current_span.record("path", path.display().to_string().as_str());
            current_span.record("content_size", parameters.content.len());

            if !self.base.is_path_allowed(path) {
                current_span.record("success", false);
                return Err(anyhow!("Path not allowed: {}", path.display()).into());
            }

            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }

            fs::write(path, &parameters.content).await?;
            current_span.record("success", true);

            Ok(format!("Wrote {} bytes to {}", parameters.content.len(), path.display()))
        }
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

impl Tool for ListDirectoryTool {
    type Params = PathParam;

    fn name() -> String {
        "list_directory".to_string()
    }

    fn description() -> String {
        "List the contents of a directory. Provide an absolute path to a directory.".to_string()
    }

    #[instrument(name = "list_directory_tool", skip(self), fields(
        tool_name = "list_directory",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        entry_count = tracing::field::Empty
    ))]
    fn call(
        &mut self,
        parameters: Self::Params
    ) -> impl std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send + Sync {
        async move {
            let current_span = tracing::Span::current();
            let path = Path::new(&parameters.path);

            current_span.record("path", path.display().to_string().as_str());

            if !self.base.is_path_allowed(path) {
                current_span.record("success", false);
                return Err(anyhow!("Path not allowed: {}", path.display()).into());
            }

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

            current_span.record("success", true);
            current_span.record("entry_count", listing.len());

            Ok(listing.join("\n"))
        }
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

impl Tool for CreateDirectoryTool {
    type Params = PathParam;

    fn name() -> String {
        "create_directory".to_string()
    }

    fn description() -> String {
        "Create a directory and all parent directories if they don't exist. Provide an absolute path.".to_string()
    }

    #[instrument(name = "create_directory_tool", skip(self), fields(
        tool_name = "create_directory",
        path = tracing::field::Empty,
        success = tracing::field::Empty
    ))]
    fn call(
        &mut self,
        parameters: Self::Params
    ) -> impl std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send + Sync {
        async move {
            let current_span = tracing::Span::current();
            let path = Path::new(&parameters.path);

            current_span.record("path", path.display().to_string().as_str());

            if !self.base.is_path_allowed(path) {
                current_span.record("success", false);
                return Err(anyhow!("Path not allowed: {}", path.display()).into());
            }

            fs::create_dir_all(path).await?;
            current_span.record("success", true);

            Ok(format!("Created directory: {}", path.display()))
        }
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

impl Tool for DeleteFileTool {
    type Params = PathParam;

    fn name() -> String {
        "delete_file".to_string()
    }

    fn description() -> String {
        "Delete a file. Provide an absolute path to the file to delete.".to_string()
    }

    #[instrument(name = "delete_file_tool", skip(self), fields(
        tool_name = "delete_file",
        path = tracing::field::Empty,
        success = tracing::field::Empty
    ))]
    fn call(
        &mut self,
        parameters: Self::Params
    ) -> impl std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send + Sync {
        async move {
            let current_span = tracing::Span::current();
            let path = Path::new(&parameters.path);

            current_span.record("path", path.display().to_string().as_str());

            if !self.base.is_path_allowed(path) {
                current_span.record("success", false);
                return Err(anyhow!("Path not allowed: {}", path.display()).into());
            }

            fs::remove_file(path).await?;
            current_span.record("success", true);

            Ok(format!("Deleted file: {}", path.display()))
        }
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

impl Tool for FileExistsTool {
    type Params = PathParam;

    fn name() -> String {
        "file_exists".to_string()
    }

    fn description() -> String {
        "Check if a file or directory exists. Provide an absolute path.".to_string()
    }

    #[instrument(name = "file_exists_tool", skip(self), fields(
        tool_name = "file_exists",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        exists = tracing::field::Empty
    ))]
    fn call(
        &mut self,
        parameters: Self::Params
    ) -> impl std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send + Sync {
        async move {
            let current_span = tracing::Span::current();
            let path = Path::new(&parameters.path);

            current_span.record("path", path.display().to_string().as_str());

            if !self.base.is_path_allowed(path) {
                current_span.record("success", false);
                return Err(anyhow!("Path not allowed: {}", path.display()).into());
            }

            let exists = fs::metadata(path).await.is_ok();
            current_span.record("success", true);
            current_span.record("exists", exists);

            Ok(format!("Exists: {}", exists))
        }
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

impl Tool for FileMetadataTool {
    type Params = PathParam;

    fn name() -> String {
        "file_metadata".to_string()
    }

    fn description() -> String {
        "Get metadata information about a file or directory (size, type, permissions). Provide an absolute path.".to_string()
    }

    #[instrument(name = "file_metadata_tool", skip(self), fields(
        tool_name = "file_metadata",
        path = tracing::field::Empty,
        success = tracing::field::Empty,
        file_size = tracing::field::Empty,
        is_dir = tracing::field::Empty
    ))]
    fn call(
        &mut self,
        parameters: Self::Params
    ) -> impl std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send + Sync {
        async move {
            let current_span = tracing::Span::current();
            let path = Path::new(&parameters.path);

            current_span.record("path", path.display().to_string().as_str());

            if !self.base.is_path_allowed(path) {
                current_span.record("success", false);
                return Err(anyhow!("Path not allowed: {}", path.display()).into());
            }

            let metadata = fs::metadata(path).await?;
            current_span.record("success", true);
            current_span.record("file_size", metadata.len());
            current_span.record("is_dir", metadata.is_dir());

            Ok(format!(
                "Size: {} bytes\nIs Directory: {}\nReadonly: {}",
                metadata.len(),
                metadata.is_dir(),
                metadata.permissions().readonly()
            ))
        }
    }
}

