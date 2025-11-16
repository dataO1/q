use ai_agent_common::*;
use async_trait::async_trait;
use anyhow::Result;

#[async_trait]
pub trait ContextProvider: Send + Sync {
    async fn gather(&self, args: Option<String>) -> Result<String>;
    fn display_name(&self) -> &str;
    fn description(&self) -> &str;
}

pub struct ContextProviderEngine {
    providers: std::collections::HashMap<String, Box<dyn ContextProvider>>,
}

impl ContextProviderEngine {
    pub fn new() -> Self {
        todo!("Register all providers")
    }

    pub async fn process_mentions(&self, input: &str) -> Result<Vec<(String, String)>> {
        todo!("Parse @ mentions and fetch context")
    }
}

// Individual providers
pub struct CurrentFileProvider;
pub struct RagProvider;
pub struct WebProvider;
pub struct GitDiffProvider;

#[async_trait]
impl ContextProvider for CurrentFileProvider {
    async fn gather(&self, _: Option<String>) -> Result<String> {
        todo!("Get current file content")
    }
    fn display_name(&self) -> &str { "@CurrentFile" }
    fn description(&self) -> &str { "Current file content" }
}
