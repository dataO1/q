use ai_agent_common::*;

pub struct ProgressiveSummarizer {
    agent: rig_core::Agent,
    summarization_interval: usize,
}

impl ProgressiveSummarizer {
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
