//! Tool integration framework
//!
//! Provides abstraction for integrating various tools like Git, LSP, filesystem operations,
//! and web scraping into agent workflows.

pub mod filesystem;
// pub mod git;
pub mod lsp;

use chrono::{DateTime, Utc};
pub use filesystem::FilesystemTool;
// pub use git::GitTool;
pub use lsp::LspTool;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ollama_rs::generation::tools::{Tool, ToolCall, ToolCallFunction, ToolFunctionInfo, ToolInfo, ToolType};
use schemars::{JsonSchema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::{Arc, RwLock}};
use std::fmt::Debug;
use derive_more::Display;
use tracing::{debug, info, error, instrument};

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
    pub fn with_result(mut self, item: &Result<String>) -> Self {
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


/// Trait alias to handle calling tools with erased Params type.
/// This matches `ToolHolder` pattern in ollama_rs for dynamic dispatch.
#[async_trait]
pub trait ToolExecutor: Send + Sync + Debug{
    /// Returns the tool name to identify it.
    fn name(&self) -> &'static str;

    /// Returns the tool description.
    fn description(&self) -> String;

    /// Calls the tool given untyped JSON parameters.
    /// Uses &self for thread-safe concurrent execution across multiple agents.
    async fn call(
        &self,
        parameters: Value,
    ) -> anyhow::Result<String>;

    /// Helper: returns ToolCall info for exposing to LLM (static params schema)
    fn provide_tool_info(&self) -> ToolInfo;
}

#[derive(Debug)]
/// ToolRegistry holds heterogeneous tool implementations behind `ToolExecutor`
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn ToolExecutor + Send + Sync>>>,
}

impl ToolRegistry {
    /// Creates new empty registry
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a new ToolExecutor implementation
    pub fn register(&self, tool: Arc<dyn ToolExecutor + Send + Sync>) {
        self.tools.write().unwrap().insert(tool.name().to_string(), tool);
    }

    /// Lookup tool executor by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolExecutor + Send + Sync>> {
        self.tools.read().unwrap().get(name).cloned()
    }

    /// Provide vector of ToolCallFunction structs for LLM prompt
    pub fn get_tools_info(&self) -> Vec<ToolInfo> {
        self.tools
            .read()
            .unwrap()
            .values()
            .map(|tool| tool.provide_tool_info())
            .collect()
    }

    /// Execute specified tool with JSON args asynchronously
    /// Uses Arc for lock-free concurrent access across multiple agents
    #[instrument(skip(self), fields(tool_name = %tool_name, args = %args))]
    pub async fn execute(&self, tool_name: &str, args: serde_json::Value) -> Result<String> {
        let tool = {
            // Acquire read lock, get the tool, and DROP the lock immediately
            let tools = self.tools.read().unwrap();
            tools.get(tool_name).cloned()
        };

        if let Some(tool) = tool {
            debug!("Executing tool '{}'", tool_name);
            
            // Execute without holding the registry lock
            // Note: ToolExecutor::call should take &self, not &mut self
            let result = tool.call(args).await;
            
            match &result {
                Ok(output) => {
                    info!("Tool '{}' completed successfully (output length: {} chars)", 
                        tool_name, output.len());
                }
                Err(e) => {
                    error!("Tool '{}' failed: {}", tool_name, e);
                }
            }
            
            result
        } else {
            error!("Tool not found: {}", tool_name);
            Err(anyhow::anyhow!("Tool not found: {}", tool_name))
        }
    }
}

// Implement ToolExecutor for all types that implement ollama_rs::Tool
// This automates wrapping of ollama Tool trait implementations.
#[async_trait]
impl<T> ToolExecutor for T
where
    T: Tool<Params = Value> + Send + Sync + Debug + Clone,
{
    fn name(&self) -> &'static str {
        // T::name() returns String, but we need &'static str
        // We leak the string to create a static reference
        Box::leak(T::name().into())
    }

    fn description(&self) -> String {
        T::description().to_string()
    }

    async fn call(
        &self,
        parameters: Value,
    ) -> anyhow::Result<String> {
        // Use Clone trait instead of unsafe pointer manipulation
        // This requires T to implement Clone, which is safer
        let mut tool_copy = self.clone();
        let result = Tool::call(&mut tool_copy, parameters).await;
        result.map_err(|e| anyhow::Error::msg(format!("Tool error: {:?}", e)))
    }

    fn provide_tool_info(&self) -> ToolInfo {
        let mut generator = SchemaGenerator::default();
        let schema = <T::Params as schemars::JsonSchema>::json_schema(&mut generator);

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: T::name().to_string(),
                description: T::description().to_string(),
                parameters: schema,
            },
        }
    }
}
