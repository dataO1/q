use ai_agent_common::*;
use fastembed::TextRerank;

pub struct FastEmbedReranker {
    model: TextRerank,
    weights: RerankingWeights,
}

impl FastEmbedReranker {
    pub fn new(weights: RerankingWeights) -> Result<Self> {
        todo!("Initialize FastEmbed reranker")
    }

    pub async fn rerank(
        &self,
        query: &str,
        documents: Vec<Document>,
        conversation_files: &[std::path::PathBuf],
    ) -> Result<Vec<Document>> {
        todo!("Rerank with semantic + boost signals")
    }
}
