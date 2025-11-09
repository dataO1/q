use ai_agent_common::*;
use rig_core::Agent;
use async_trait::async_trait;
use std::sync::Arc;

/// Planning agent for strategy, architecture, task sequencing
pub struct PlanningAgent {
    agent: Arc<Agent>,
}

impl PlanningAgent {
    pub fn new(agent: Arc<Agent>) -> Self {
        Self { agent }
    }

    pub async fn plan(&self, task: &str, context: &str) -> Result<String> {
        todo!("Implement planning logic")
    }
}

#[async_trait]
impl Agent for PlanningAgent {
    async fn prompt(&self, input: &str) -> Result<String> {
        self.plan(input, "").await
    }
}
