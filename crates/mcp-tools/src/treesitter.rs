use ai_agent_common::*;
use mcp_core::{Tool, Content, ToolError};
use async_trait::async_trait;

pub struct TreeSitterTool;

impl TreeSitterTool {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for TreeSitterTool {
    fn name(&self) -> &str {
        "treesitter"
    }

    fn description(&self) -> &str {
        "Parse code using tree-sitter"
    }

    async fn call(&self, args: serde_json::Value) -> Result<Vec<Content>, ToolError> {
        todo!("Implement tree-sitter parsing tool")
    }
}
