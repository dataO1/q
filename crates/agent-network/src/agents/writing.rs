//! Writing agent implementation

use crate::{
    agents::{Agent, AgentType, AgentContext, AgentResponse},
};
use crate::error::{AgentNetworkError, AgentNetworkResult};
use async_trait::async_trait;

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
    async fn execute(&self, context: AgentContext) -> AgentNetworkResult<AgentResponse> {
        tracing::info!("WritingAgent executing task: {}", context.task_id);

        // TODO: Week 3 - Implement writing agent logic
        // - Generate documentation
        // - Write commit messages
        // - Create reports

        Ok(AgentResponse {
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
