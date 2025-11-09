//! MCP Tool Implementations

pub mod lsp;
pub mod treesitter;
pub mod git;
pub mod filesystem;
pub mod web;

use ai_agent_common::*;
use mcp_core::{Tool, Content, ToolError};

pub fn register_all_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(lsp::LspTool::new()),
        Box::new(treesitter::TreesitterTool::new()),
        Box::new(git::GitTool::new()),
        Box::new(filesystem::FileSystemTool::new()),
        Box::new(web::WebSearchTool::new()),
    ]
}
