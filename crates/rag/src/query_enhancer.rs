use ai_agent_common::*;

pub struct QueryEnhancer {
    // Using String to store model name for now
    // Will initialize actual client when implementing
    model_name: String,
}

impl QueryEnhancer {
    pub fn new(model: &str) -> Result<Self> {
        Ok(Self {
            model_name: model.to_string(),
        })
    }

    pub async fn enhance_with_history(
        &self,
        query: &str,
        history_context: &str,
    ) -> Result<String> {
        todo!("Rewrite query with history context using LLM")
    }

    pub async fn generate_source_specific_queries(
        &self,
        query: &str,
        sources: &[CollectionTier],
    ) -> Result<Vec<(CollectionTier, String)>> {
        todo!("Generate optimized query per source")
    }
}
