use ai_agent_common::*;
use async_trait::async_trait;
use crate::Tool;

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

    async fn call(&self, args: serde_json::Value) -> anyhow::Result<String> {
        todo!("Execute git commands")
    }
}
