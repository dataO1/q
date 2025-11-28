use serde::{Deserialize, Serialize};
use serde_json::Value;
use schemars::JsonSchema;
use anyhow::{Result, anyhow};
use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};
pub mod filesystem;
// pub mod git;
pub mod lsp;

use chrono::{DateTime, Utc};

// Re-export the tool implementations
pub use filesystem::{
    WriteFileTool, ReadFileTool, ListDirectoryTool,
    CreateDirectoryTool, FileExistsTool, FileMetadataTool,
    DeleteFileTool
};
pub use lsp::LspTool;

use crate::tools::filesystem::FILESYSTEM_PREAMBLE;
// Simple result type for tool execution
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
}

// Tool execution metadata for tracking and logging
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolExecution {
    pub tool_name: String,
    pub arguments: String,
    pub result: ToolResult,
    pub execution_time_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Core trait - fully dyn-compatible
#[async_trait::async_trait]
pub trait Tool: Send + Sync + std::fmt::Debug{
    /// Execute the tool with JSON arguments (as string or Value)
    async fn call(&self, arguments: &str) -> Result<ToolResult>;

    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get the JSON schema for the tool's parameters
    fn parameters(&self) -> Value;

    /// Get the ChatCompletionTool definition for this tool (default implementation)
    fn to_openai_tool(&self) -> ChatCompletionTool {
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: self.name().to_string(),
                description: Some(self.description().to_string()),
                parameters: Some(self.parameters()),
                strict: Some(true),
            },
        }
    }
}

/// Helper trait for tools that use typed parameters
/// This provides the generic method, but constrained to Sized types
#[async_trait::async_trait]
pub trait TypedTool: Send + Sync {
    /// The parameter struct for this tool
    type Params: JsonSchema + for<'de> Deserialize<'de> + Send;

    /// Execute with typed parameters
    async fn call(&self, params: Self::Params) -> Result<ToolResult>;

    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Helper to generate schema (only available on concrete types)
    fn schema_for_params() -> Value where Self: Sized {
        let schema = schemars::schema_for!(Self::Params);
        serde_json::to_value(schema).unwrap_or_default()
    }
}

/// Blanket implementation: any TypedTool automatically becomes a Tool
#[async_trait::async_trait]
impl<T> Tool for T
where
    T: TypedTool + std::fmt::Debug,
{
    async fn call(&self, arguments: &str) -> Result<ToolResult> {
        let params: T::Params = serde_json::from_str(arguments)
            .map_err(|e| anyhow!("Failed to parse arguments for {}: {}", self.name(), e))?;
        self.call(params).await
    }

    fn name(&self) -> &str {
        TypedTool::name(self)
    }

    fn description(&self) -> &str {
        TypedTool::description(self)
    }

    fn parameters(&self) -> Value {
        let schema = schemars::schema_for!(T::Params);
        serde_json::to_value(schema).unwrap_or_default()
    }
}

/// Collection of available tools
#[derive(Debug)]
pub struct ToolSet {
    tools: std::collections::HashMap<String, Box<dyn Tool>>,
}

impl ToolSet {
    pub fn new(base_path: &str) -> Self {
        let mut tools = std::collections::HashMap::new();

        // Register available tools
        tools.insert(
            "write_file".to_string(),
            Box::new(WriteFileTool::new(base_path)) as Box<dyn Tool>,
        );
        // ... other tools

        Self { tools }
    }

    pub fn register_tool<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        self.tools.insert(name, Box::new(tool));
    }

    pub fn available_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn to_openai_tools(&self, required_tools: &[String]) -> Result<Vec<ChatCompletionTool>> {
        let tools = required_tools
            .iter()
            .filter_map(|name| self.tools.get(name).map(|t| t.to_openai_tool()))
            .collect::<Vec<_>>();
        Ok(tools)
    }

    pub async fn execute_tool(&self, tool_name: &str, arguments: &str) -> Result<ToolExecution> {
        if let Some(tool) = self.tools.get(tool_name) {
            let start_time = std::time::Instant::now();
            let timestamp = chrono::Utc::now();

            let result = tool.call(arguments).await?;
            let execution_time_ms = start_time.elapsed().as_millis() as u64;

            Ok(ToolExecution {
                tool_name: tool_name.to_string(),
                arguments: arguments.to_string(),
                result,
                execution_time_ms,
                timestamp,
            })
        } else {
            Err(anyhow!("Unknown tool: {}", tool_name))
        }
    }

    pub fn get_tool_type_instructions(&self, tool_name: &str) -> Option<String> {
        // TODO: implement this really
        return Some(FILESYSTEM_PREAMBLE.to_string())
    }
}
