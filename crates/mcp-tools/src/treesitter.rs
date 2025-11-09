use async_trait::async_trait;

use crate::Tool;

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

    async fn call(&self, args: serde_json::Value) -> anyhow::Result<String> {
        todo!("Implement tree-sitter parsing tool")
    }
}
