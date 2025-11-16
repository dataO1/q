//! Writing agent for documentation and communication
//!
//! Generates documentation, commit messages, and communication.

use crate::{agents::{Agent, AgentContext, AgentResult, AgentType}, error::AgentNetworkResult};
use async_trait::async_trait;
use tracing::{info, instrument};

pub struct WritingAgent {
    id: String,
    model: String,
    system_prompt: String,
    temperature: f32,
    max_tokens: usize,
}

impl WritingAgent {
    pub fn new(
        id: String,
        model: String,
        system_prompt: String,
        temperature: f32,
        max_tokens: usize,
    ) -> Self {
        Self {
            id,
            model,
            system_prompt,
            temperature: temperature.clamp(0.0, 2.0),
            max_tokens,
        }
    }
}

#[async_trait]
impl Agent for WritingAgent {
    #[instrument(skip(self, context))]
    async fn execute(&self, context: AgentContext) -> AgentNetworkResult<AgentResult> {
        info!("Writing agent executing task: {}", context.task_id);

        let output = format!(
            "# Documentation: {}\n\n\
             ## Overview\n{}\n\n\
             ## Details\nProviding comprehensive documentation.\n\n\
             ## Usage\nInstructions for usage.",
            context.task_id,
            context.description
        );

        Ok(AgentResult::new(self.id.clone(), output)
            .with_confidence(0.85))
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn agent_type(&self) -> AgentType {
        AgentType::Writing
    }

    fn description(&self) -> &str {
        "Technical writing expert for documentation and communication"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_writing_agent() {
        let agent = WritingAgent::new(
            "writing-1".to_string(),
            "model".to_string(),
            "prompt".to_string(),
            0.7,
            2048,
        );

        let context = AgentContext::new(
            "task-1".to_string(),
            "Write module documentation".to_string(),
        );

        let result = agent.execute(context).await;
        assert!(result.is_ok());
    }
}
