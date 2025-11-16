use ai_agent_common::*;
use crate::*;
use anyhow::Result;

#[derive(Debug)]
pub struct HistoryManager {
    // buffer_memory: buffer_memory::BufferMemory,
    // semantic_memory: semantic_memory::SemanticMemory,
    // summarizer: summarizer::ProgressiveSummarizer,
    // metadata_tracker: metadata::MetadataTracker,
}

impl HistoryManager {
    pub async fn new(postgres_url: &str, config: &RagConfig) -> Result<Self> {
        return Ok(Self{})
    }

    pub async fn add_exchange(
        &mut self,
        conversation_id: &ConversationId,
        user_query: String,
        agent_response: String
    ) -> Result<()> {
        Ok(())
    }

    pub async fn get_relevant_context(
        &self,
        conversation_id: &ConversationId,
        query: String,
    ) -> Result<HistoryContext> {
        Ok(HistoryContext::default())
    }
}

#[derive(Debug, Clone)]
pub struct HistoryContext {
    pub short_term: Vec<Message>,
    pub relevant_past: Vec<Message>,
    pub summary: Option<String>,
    pub mentioned_files: Vec<std::path::PathBuf>,
    pub topics: Vec<String>,
}

impl Default for HistoryContext{
    fn default() -> Self{
        Self{
            short_term: vec![],
            relevant_past: vec![],
            summary : None,
            mentioned_files: vec![],
            topics: vec![]
        }
    }
}
