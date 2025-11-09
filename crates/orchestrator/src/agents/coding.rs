use ai_agent_common::*;
use rig_core::Agent;
use async_trait::async_trait;
use std::sync::Arc;

/// Coding agent specialized for code generation and fixes
pub struct CodingAgent {
    agent: Arc<Agent>,
}

impl CodingAgent {
    pub fn new(agent: Arc<Agent>) -> Self {
        Self { agent }
    }

    pub async fn generate_code(&self, task: &str, context: &str) -> Result<String> {
        todo!("Implement code generation logic")
    }
}

#[async_trait]
impl Agent for CodingAgent {
    async fn prompt(&self, input: &str) -> Result<String> {
        self.generate_code(input, "").await
    }
}
