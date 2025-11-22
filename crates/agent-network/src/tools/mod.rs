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
use std::{collections::HashMap, future::Future, pin::Pin};
use futures::{FutureExt, TryFutureExt};
use std::fmt::Debug;
use derive_more::Display;

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
    async fn call(
        &mut self,
        parameters: Value,
    ) -> anyhow::Result<String>;

    /// Helper: returns ToolCall info for exposing to LLM (static params schema)
    fn provide_tool_info(&self) -> ToolInfo;
}

#[derive(Debug)]
/// ToolRegistry holds heterogeneous tool implementations behind `ToolExecutor`
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolExecutor>>,
}

impl ToolRegistry {
    /// Creates new empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Registers a new ToolExecutor implementation
    pub fn register(&mut self, tool: Box<dyn ToolExecutor>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Lookup tool executor by name
    pub fn get(&self, name: &str) -> Option<&dyn ToolExecutor> {
        self.tools.get(name).map(|b| &**b)
    }

    /// Provide vector of ToolCallFunction structs for LLM prompt
    pub fn get_tools_info(&self) -> Vec<ToolInfo> {
        self.tools
            .values()
            .map(|tool| tool.provide_tool_info())
            .collect()
    }

    /// Execute specified tool with JSON args asynchronously
    pub async fn execute(&mut self, name: &str, args: Value) -> Result<String> {
        let tool = self
            .tools
            .get_mut(name)
            .ok_or_else(|| anyhow!("Tool '{}' not found", name))?;

        tool.call(args.clone()).await
    }
}

// Implement ToolExecutor for all types that implement ollama_rs::Tool
// This automates wrapping of ollama Tool trait implementations.
#[async_trait]
impl<T> ToolExecutor for T
where
    T: Tool<Params = Value> + Send + Sync + Debug,
{
    fn name(&self) -> &'static str {
        T::name()
    }

    fn description(&self) -> &'static str {
        T::description()
    }

    async fn call(
        &mut self,
        parameters: Value,
    ) -> anyhow::Result<String> {
        // Await the future from Tool::call, map error directly
        Tool::call(self, parameters)
            .await
            .map_err(|e| anyhow::Error::msg(format!("Tool error: {:?}", e)))
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
