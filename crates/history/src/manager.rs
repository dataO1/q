use ai_agent_common::*;
use rig::completion::CompletionModel;
use crate::*;

pub struct HistoryManager<M:CompletionModel> {
    buffer_memory: buffer_memory::BufferMemory,
    semantic_memory: semantic_memory::SemanticMemory,
    summarizer: summarizer::ProgressiveSummarizer<M>,
    metadata_tracker: metadata::MetadataTracker,
}

impl<M:CompletionModel> HistoryManager<M> {
    pub async fn new(postgres_url: &str, config: &RagConfig) -> Result<Self> {
        todo!("Initialize history manager")
    }

    pub async fn add_exchange(
        &mut self,
        conversation_id: &ConversationId,
        user_query: &str,
        agent_response: &str,
    ) -> Result<()> {
        todo!("Store new exchange in all layers")
    }

    pub async fn get_relevant_context(
        &self,
        conversation_id: &ConversationId,
        query: &str,
    ) -> Result<HistoryContext> {
        todo!("Retrieve: short-term + semantic search + summaries")
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
