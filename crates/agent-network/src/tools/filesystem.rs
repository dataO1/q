//! Filesystem tool integration

use crate::{
    tools::{Tool, ToolInput, ToolOutput},
};
use crate::error::AgentNetworkResult;
use async_trait::async_trait;

pub struct FilesystemTool {
    name: String,
}

impl FilesystemTool {
    pub fn new() -> Self {
        Self {
            name: "filesystem".to_string(),
        }
    }
}

impl Default for FilesystemTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FilesystemTool {
    async fn execute(&self, input: ToolInput) -> AgentNetworkResult<ToolOutput> {
        tracing::debug!("Filesystem tool executing: {} {:?}", input.command, input.args);

        // TODO: Week 4 - Implement filesystem tool integration
        // - Read/write files
        // - List directories
        // - Respect .gitignore

        Ok(ToolOutput {
            stdout: "Filesystem tool output".to_string(),
            stderr: String::new(),
            success: true,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}
