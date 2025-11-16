//! LSP tool integration

use crate::{
    tools::{Tool, ToolInput, ToolOutput},
};
use crate::error::AgentResult;
use async_trait::async_trait;

pub struct LspTool {
    name: String,
}

impl LspTool {
    pub fn new() -> Self {
        Self {
            name: "lsp".to_string(),
        }
    }
}

impl Default for LspTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for LspTool {
    async fn execute(&self, input: ToolInput) -> AgentResult<ToolOutput> {
        tracing::debug!("LSP tool executing: {} {:?}", input.command, input.args);

        // TODO: Week 4 - Implement LSP tool integration
        // - Connect to LSP servers
        // - Execute LSP commands
        // - Return results

        Ok(ToolOutput {
            stdout: "LSP tool output".to_string(),
            stderr: String::new(),
            success: true,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}
