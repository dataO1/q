use ai_agent_common::*;
use mcp_core::{Tool, Content, ToolError};
use async_trait::async_trait;

pub struct GitTool;

impl GitTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GitTool {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Git operations: log, blame, diff, commit"
    }

    async fn call(&self, args: serde_json::Value) -> std::result::Result<Vec<Content>, ToolError> {
        todo!("Execute git commands")
    }
}
