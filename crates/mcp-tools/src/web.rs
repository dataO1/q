use async_trait::async_trait;

use crate::Tool;

pub struct WebSearchTool;

impl WebSearchTool {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search online documentation and resources"
    }

    async fn call(&self, args: serde_json::Value) -> anyhow::Result<String> {
        todo!("Implement web search via APIs")
    }
}
