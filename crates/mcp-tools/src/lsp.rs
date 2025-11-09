use async_trait::async_trait;
use crate::Tool;

pub struct LspTool;

impl LspTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        "lsp_query"
    }

    fn description(&self) -> &str {
        "Query LSP server for code definitions, references, types"
    }

    async fn call(&self, args: serde_json::Value) -> anyhow::Result<String> {
        todo!("Execute LSP query")
    }
}
