//! Tool integration framework
//!
//! Provides abstraction for integrating various tools like Git, LSP, filesystem operations,
//! and web scraping into agent workflows.

pub mod filesystem;
pub mod git;
pub mod lsp;
pub mod web;

pub use filesystem::FilesystemTool;
pub use git::GitTool;
pub use lsp::LspTool;
// pub use web::WebTool;

use crate::{agents::ToolResult, error::AgentNetworkResult};
use async_trait::async_trait;
use std::collections::HashMap;

/// Trait for all tools that agents can use
#[async_trait]
pub trait Tool: Send + Sync {
    /// Execute the tool with given parameters
    async fn execute(&self, command: &str, params: HashMap<String, String>) -> AgentNetworkResult<ToolResult>;

    /// Get tool name
    fn name(&self) -> &str;

    /// Get tool description
    fn description(&self) -> &str;

    /// Get available commands for this tool
    fn available_commands(&self) -> Vec<String>;

    /// Validate parameters before execution
    fn validate_params(&self, command: &str, params: &HashMap<String, String>) -> AgentNetworkResult<()>;
}

/// Tool registry for managing multiple tools
pub struct ToolRegistry {
    tools: HashMap<String, std::sync::Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: std::sync::Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<std::sync::Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// List all registered tools
    pub fn list(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get tool by name and execute command
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        command: &str,
        params: HashMap<String, String>,
    ) -> AgentNetworkResult<ToolResult> {
        let tool = self
            .get(tool_name)
            .ok_or_else(|| crate::error::AgentNetworkError::Tool(
                format!("Tool not found: {}", tool_name),
            ))?;

        tool.validate_params(command, &params)?;
        tool.execute(command, params).await
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registry_creation() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.list().len(), 0);
    }
}
