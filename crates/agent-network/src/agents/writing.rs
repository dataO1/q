//! Writing agent implementation

use crate::{
    agents::{Agent, AgentType, AgentContext, AgentResult},
};
use async_trait::async_trait;
use ai_agent_common::types::Result;

pub struct WritingAgent {
    id: String,
    model: String,
}

impl WritingAgent {
    pub fn new(id: String, model: String) -> Self {
        Self { id, model }
    }
}

#[async_trait]
impl Agent for WritingAgent {
    async fn execute(&self, context: AgentContext) -> Result<AgentResult> {
        tracing::info!("WritingAgent executing task: {}", context.task_id);

        // TODO: Week 3 - Implement writing agent logic
        // - Generate documentation
        // - Write commit messages
        // - Create reports

        Ok(AgentResult {
            agent_id: self.id.clone(),
            output: "Writing task completed".to_string(),
            confidence: 0.9,
            requires_hitl: false,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn agent_type(&self) -> AgentType {
        AgentType::Writing
    }
}
