use ai_agent_common::*;
use rig::agent::Agent;
use rig::completion::CompletionModel;
use std::sync::Arc;

/// Coding agent specialized for code generation and fixes
pub struct CodingAgent<M:CompletionModel> {
    agent: Arc<Agent<M>>,
}

impl<M:CompletionModel> CodingAgent<M> {
    pub fn new(agent: Arc<Agent<M>>) -> Self {
        Self { agent }
    }

    pub async fn generate_code(&self, task: &str, context: &str) -> Result<String> {
        todo!("Implement code generation logic")
    }
}

// #[async_trait]
// impl Agent for CodingAgent {
//     async fn prompt(&self, input: &str) -> Result<String> {
//         self.generate_code(input, "").await
//     }
// }
