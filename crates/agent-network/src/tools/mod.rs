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

use crate::tools::filesystem::FILESYSTEM_PREAMBLE;

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


/// Collection of available tools
#[derive(Debug, Clone)]
pub struct ToolSet {
    pub write_file: WriteFileTool,
    pub read_file: ReadFileTool,
    pub list_directory: ListDirectoryTool,
    pub create_directory: CreateDirectoryTool,
    pub file_exists: FileExistsTool,
    pub file_metadata: FileMetadataTool,
    pub delete_file: DeleteFileTool,
    // pub lsp: Arc<LspTool>,
}

impl ToolSet {
    pub fn new(path: &str) -> Self {
        Self {
            write_file: WriteFileTool::new(path),
            read_file: ReadFileTool::new(path),
            list_directory: ListDirectoryTool::new(path),
            create_directory: CreateDirectoryTool::new(path),
            file_exists: FileExistsTool::new(path),
            file_metadata: FileMetadataTool::new(path),
            delete_file: DeleteFileTool::new(path),
            // lsp: Arc::new(LspTool::new()),
        }
    }


    /// Get all available tool names
    pub fn available_tools(&self) -> Vec<String> {
        vec![
            "write_file".to_string(),
            "read_file".to_string(),
            "list_directory".to_string(),
            "create_directory".to_string(),
            "file_exists".to_string(),
            "file_metadata".to_string(),
            "delete_file".to_string(),
            "lsp".to_string(),
        ]
    }

    pub fn get_tool_type_instructions(&self,tool_name: &str ) ->Option<String>{
        // TODO: implement this really
        return Some(FILESYSTEM_PREAMBLE.to_string())
    }
}

