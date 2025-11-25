//! Tool integration framework
//!
//! Provides native ollama_rs::Tool implementations for agent workflows.

pub mod filesystem;
// pub mod git;
pub mod lsp;

use chrono::{DateTime, Utc};
pub use filesystem::{WriteFileTool, ReadFileTool, ListDirectoryTool, CreateDirectoryTool, FileExistsTool, FileMetadataTool, DeleteFileTool};
// pub use git::GitTool;
pub use lsp::LspTool;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use derive_more::Display;
use schemars::JsonSchema;
use anyhow;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Display)]
#[display("Name: {}, Params: {}, Result: {:?}, Error: {:?}, timestamp: {}", tool_name, parameters, result, error, timestamp)]
pub struct ToolExecution {
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub result: Option<String>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

// Implement From<i32> for Number (an infallible conversion)
impl ToolExecution {
    pub fn new(name: &str, args: &Value)-> Self{
        Self{
            tool_name: name.to_string(),
            parameters: args.clone(),
            result: None,
            error: None,
            timestamp: Utc::now(),
        }
    }
    pub fn with_result(mut self, item: &anyhow::Result<String>) -> Self {
        match item{
            Ok(result) =>{
                self.result = Some(result.clone());
            },
            Err(err) =>{
                self.error = Some(err.to_string());
            }
        }
        self.timestamp = Utc::now();
        self
    }
}

/// Enum to hold different tool types for dynamic access
#[derive(Debug, Clone)]
pub enum DynamicTool {
    WriteFile(Arc<WriteFileTool>),
    ReadFile(Arc<ReadFileTool>),
    ListDirectory(Arc<ListDirectoryTool>),
    CreateDirectory(Arc<CreateDirectoryTool>),
    FileExists(Arc<FileExistsTool>),
    FileMetadata(Arc<FileMetadataTool>),
    DeleteFile(Arc<DeleteFileTool>),
    Lsp(Arc<LspTool>),
}

impl DynamicTool {
    /// Convert to the actual tool reference that coordinator expects
    pub fn as_tool_ref(&self) -> Box<dyn std::any::Any + Send + Sync> {
        match self {
            DynamicTool::WriteFile(tool) => Box::new(tool.as_ref().clone()),
            DynamicTool::ReadFile(tool) => Box::new(tool.as_ref().clone()),
            DynamicTool::ListDirectory(tool) => Box::new(tool.as_ref().clone()),
            DynamicTool::CreateDirectory(tool) => Box::new(tool.as_ref().clone()),
            DynamicTool::FileExists(tool) => Box::new(tool.as_ref().clone()),
            DynamicTool::FileMetadata(tool) => Box::new(tool.as_ref().clone()),
            DynamicTool::DeleteFile(tool) => Box::new(tool.as_ref().clone()),
            DynamicTool::Lsp(tool) => Box::new(tool.as_ref().clone()),
        }
    }
}

/// Collection of available tools
#[derive(Debug, Clone)]
pub struct ToolSet {
    pub write_file: Arc<WriteFileTool>,
    pub lsp: Arc<LspTool>,
    // HashMap for dynamic tool access
    pub tools: HashMap<String, DynamicTool>,
    // Base path for creating filesystem tools
    pub base_path: String,
}

impl ToolSet {
    pub fn new(path: &str) -> Self {
        Self {
            write_file: Arc::new(WriteFileTool::new(path)),
            lsp: Arc::new(LspTool::new()),
            tools: HashMap::new(),
            base_path: path.to_string(),
        }
    }

    /// Get a tool by name for dynamic assignment
    pub fn get_tool(&self, name: &str) -> Option<&DynamicTool> {
        self.tools.get(name)
    }

    /// Create and add filesystem tools based on name
    pub fn ensure_filesystem_tool(&mut self, tool_name: &str) {
        if self.tools.contains_key(tool_name) {
            return; // Tool already exists
        }

        match tool_name {
            "write_file" => {
                let tool = Arc::new(WriteFileTool::new(&self.base_path));
                self.tools.insert(tool_name.to_string(), DynamicTool::WriteFile(tool));
            }
            "read_file" => {
                let tool = Arc::new(ReadFileTool::new(&self.base_path));
                self.tools.insert(tool_name.to_string(), DynamicTool::ReadFile(tool));
            }
            "list_directory" => {
                let tool = Arc::new(ListDirectoryTool::new(&self.base_path));
                self.tools.insert(tool_name.to_string(), DynamicTool::ListDirectory(tool));
            }
            "create_directory" => {
                let tool = Arc::new(CreateDirectoryTool::new(&self.base_path));
                self.tools.insert(tool_name.to_string(), DynamicTool::CreateDirectory(tool));
            }
            "file_exists" => {
                let tool = Arc::new(FileExistsTool::new(&self.base_path));
                self.tools.insert(tool_name.to_string(), DynamicTool::FileExists(tool));
            }
            "file_metadata" => {
                let tool = Arc::new(FileMetadataTool::new(&self.base_path));
                self.tools.insert(tool_name.to_string(), DynamicTool::FileMetadata(tool));
            }
            "delete_file" => {
                let tool = Arc::new(DeleteFileTool::new(&self.base_path));
                self.tools.insert(tool_name.to_string(), DynamicTool::DeleteFile(tool));
            }
            "lsp" => {
                let tool = Arc::new(LspTool::new());
                self.tools.insert(tool_name.to_string(), DynamicTool::Lsp(tool));
            }
            _ => {
                // Unknown tool - could log a warning or handle gracefully
            }
        }
    }

    /// Get all available tool names
    pub fn available_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

