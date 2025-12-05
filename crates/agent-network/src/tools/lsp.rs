use ai_agent_common::{HitlMetadata, HitlPreview, HitlRequest};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tracing::debug;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use crate::tools::{Tool, ToolResult, TypedTool};

#[derive(Debug, Clone)]
pub struct LspTool {
}

impl LspTool {
    pub fn new() -> Self {
        debug!("LspTool initialized for language");
        Self {}
    }

    // Placeholder implementations, replace with actual tower-lsp async client logic

    async fn completions(&self, context: &str) -> Result<String> {
        // TODO: integrate tower-lsp completion request
        Ok(format!("Completions for context: {}", &context[..context.len().min(100)]))
    }

    async fn diagnostics(&self, code: &str) -> Result<String> {
        // TODO: get diagnostics from language server
        Ok("No diagnostics".to_string())
    }

    async fn symbols(&self, code: &str) -> Result<String> {
        // TODO: analyze symbols via language server
        Ok("Symbols: [placeholder]".to_string())
    }

    async fn type_info(&self, code: &str, position: usize) -> Result<String> {
        // TODO: get type info at position
        Ok(format!("Type info at position {}: unknown", position))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LspParams {
    pub command: String,
    pub context: Option<String>,
    pub code: Option<String>,
    pub position: Option<usize>,
}

#[async_trait::async_trait]
impl TypedTool for LspTool {
    type Params = LspParams;
    fn name(&self) -> &str {
        "lsp"
    }

    fn description(&self) -> &str {
        "Language Server Protocol tool for code analysis and completion"
    }

    async fn hitl_request(&self, params: Self::Params) -> Result<HitlRequest>{
        let metadata = HitlMetadata{
            file_path: None,
            file_size: None,
            is_new_file: false,
            is_destructive: false,
            requires_network: false,
        };

        let preview = HitlPreview::None; // TODO:: add filecontent preview of the file to delete
        Ok(HitlRequest{preview, metadata})
    }

    async fn call(&self, params: Self::Params) -> Result<ToolResult> {
        let result = match params.command.as_str() {
                "completions" => {
                    let context = params.context.unwrap_or_default();
                    self.completions(&context).await?
                }
                "diagnostics" => {
                    let code = params.code.unwrap_or_default();
                    self.diagnostics(&code).await?
                }
                "symbols" => {
                    let code = params.code.unwrap_or_default();
                    self.symbols(&code).await?
                }
                "type_info" => {
                    let code = params.code.unwrap_or_default();
                    let position = params.position.unwrap_or(0);
                    self.type_info(&code, position).await?
                }
                _ => return Err(anyhow!("Unknown LSP command: {}", params.command)),
            };

        Ok(ToolResult {
            success: true,
            output: result,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lsp_tool_call() {
        let tool = LspTool::new();
        let params = LspParams {
            command: "completions".to_string(),
            context: Some("fn main() { pri".to_string()),
            code: None,
            position: None,
        };
        let result = tool.call(params).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Completions"));
    }
}
