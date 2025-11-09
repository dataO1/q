use ai_agent_common::*;
use rig::agent::Agent;
use rig::completion::CompletionModel;

pub struct ProgressiveSummarizer<M:CompletionModel>{
    agent: Agent<M>,
    summarization_interval: usize,
}

impl<M:CompletionModel> ProgressiveSummarizer<M> {
    pub fn new(model: &str, interval: usize) -> Result<Self> {
        todo!("Initialize summarization agent")
    }

    pub async fn should_summarize(&self, message_count: usize) -> bool {
        message_count % self.summarization_interval == 0
    }

    pub async fn summarize_chunk(&self, messages: &[Message]) -> Result<String> {
        todo!("Generate summary with LLM")
    }
}
