use ai_agent_common::*;
use rig::agent::Agent;
use rig::completion::CompletionModel;
use std::sync::Arc;

/// Writing agent specialized in documentation and explanations
pub struct WritingAgent<M:CompletionModel> {
    agent: Arc<Agent<M>>,
}

impl<M:CompletionModel> WritingAgent<M> {
    pub fn new(agent: Arc<Agent<M>>) -> Self {
        Self { agent }
    }

    pub async fn write(&self, task: &str, context: &str) -> Result<String> {
        todo!("Implement writing logic")
    }
}

// #[async_trait]
// impl<M: CompletionModel> Chat for WritingAgent<M> {
//     async fn chat(&self, input: &str, history: Vec<Message>) -> Result<String> {
//         // Use history to provide context
//         let context = self.format_history(&history);
//         self.write(input, &context).await
//     }
// }
