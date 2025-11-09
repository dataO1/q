use ai_agent_common::*;
use mcp_core::{Tool, Content, ToolError};
use async_trait::async_trait;

pub struct FileSystemTool;

impl FileSystemTool {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for FileSystemTool {
    fn name(&self) -> &str {
        "filesystem"
    }

    fn description(&self) -> &str {
        "File system operations (read, write, list)"
    }

    async fn call(&self, args: serde_json::Value) -> Result<Vec<Content>, ToolError> {
        todo!("Implement filesystem tool commands")
    }
}
