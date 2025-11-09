use ai_agent_common::*;
use rig_core::Agent;
use async_trait::async_trait;
use std::sync::Arc;

/// Writing agent specialized in documentation and explanations
pub struct WritingAgent {
    agent: Arc<Agent>,
}

impl WritingAgent {
    pub fn new(agent: Arc<Agent>) -> Self {
        Self { agent }
    }

    pub async fn write(&self, task: &str, context: &str) -> Result<String> {
        todo!("Implement writing logic")
    }
}

#[async_trait]
impl Agent for WritingAgent {
    async fn prompt(&self, input: &str) -> Result<String> {
        self.write(input, "").await
    }
}
