use ai_agent_common::*;
use rig::agent::Agent;
use rig::completion::CompletionModel;
use std::sync::Arc;

/// Planning agent for strategy, architecture, task sequencing
pub struct PlanningAgent<M:CompletionModel> {
    agent: Arc<Agent<M>>,
}

impl<M:CompletionModel> PlanningAgent<M> {
    pub fn new(agent: Arc<Agent<M>>) -> Self {
        Self { agent }
    }

    pub async fn plan(&self, task: &str, context: &str) -> Result<String> {
        todo!("Implement planning logic")
    }
}

// #[async_trait]
// impl Agent for PlanningAgent {
//     async fn prompt(&self, input: &str) -> Result<String> {
//         self.plan(input, "").await
//     }
// }
