//! Coding agent implementation

use crate::{
    agents::{Agent, AgentType, AgentContext, AgentResult},
};
use async_trait::async_trait;
use ai_agent_common::types::Result;

pub struct CodingAgent {
    id: String,
    model: String,
    // TODO: Week 3 - Add Rig integration
}

impl CodingAgent {
    pub fn new(id: String, model: String) -> Self {
        Self { id, model }
    }
}

#[async_trait]
impl Agent for CodingAgent {
    async fn execute(&self, context: AgentContext) -> Result<AgentResult> {
        tracing::info!("CodingAgent executing task: {}", context.task_id);

        // TODO: Week 3 - Implement coding agent logic
        // - Integrate with Rig
        // - Use LSP tools
        // - Generate code
        // - Handle file operations

        Ok(AgentResult {
            agent_id: self.id.clone(),
            output: "Coding task completed".to_string(),
            confidence: 0.9,
            requires_hitl: false,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn agent_type(&self) -> AgentType {
        AgentType::Coding
    }
}
