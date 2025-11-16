//! Tool integrations via MCP

pub mod lsp;
pub mod git;
pub mod filesystem;
pub mod treesitter;

use crate::error::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Tool: Send + Sync {
    async fn execute(&self, input: ToolInput) -> Result<ToolOutput>;
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct ToolInput {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}
