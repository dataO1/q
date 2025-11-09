pub mod lsp;
pub mod treesitter;
pub mod git;
pub mod filesystem;
pub mod web;

use async_trait::async_trait;
use serde_json::Value;

/// Tool trait for MCP tools (temporary until mcp_core stabilizes)
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn call(&self, args: Value) -> anyhow::Result<String>;
}

pub fn register_all_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(lsp::LspTool::new()),
        Box::new(treesitter::TreeSitterTool::new()),
        Box::new(git::GitTool::new()),
        Box::new(filesystem::FileSystemTool::new()),
        Box::new(web::WebSearchTool::new()),
    ]
}
