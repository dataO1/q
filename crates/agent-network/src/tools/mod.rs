//! Tool integration framework
//!
//! Provides native ollama_rs::Tool implementations for agent workflows.

pub mod filesystem;
// pub mod git;
pub mod lsp;

use chrono::{DateTime, Utc};
pub use filesystem::WriteFileTool;
// pub use git::GitTool;
pub use lsp::LspTool;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use derive_more::Display;
use schemars::JsonSchema;
use anyhow;

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
    pub write_file: std::sync::Arc<WriteFileTool>,
    pub lsp: std::sync::Arc<LspTool>,
}

impl ToolSet {
    pub fn new(path: &str) -> Self {
        Self {
            write_file: std::sync::Arc::new(WriteFileTool::new(path)),
            lsp: std::sync::Arc::new(LspTool::new()),
        }
    }
}

