// use crate::config::OllamaConfig;
use crate::Config;
use anyhow::{Result};
use qdrant_client::{qdrant::ScoredPoint, Qdrant};
use qdrant_client::qdrant::SearchPointsBuilder;
use serde::{Deserialize, Serialize};
use swiftide_integrations::ollama::Ollama;
use swiftide::traits::EmbeddingModel;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
    pub snippet: String,
}

// Implement the From trait for the conversion
impl From<ScoredPoint> for SearchResult {
    fn from(item: ScoredPoint) -> Self {
        return SearchResult {
            path: item.get("path").to_string(),
            score: item.score ,
            snippet: item.get("content").to_string() };
    }
}

pub async fn search(
    query: &str,
    ollama: &Ollama,
    client: &Qdrant,
    config: &Config,
) -> Result<Vec<SearchResult>> {
    let limit = config.qdrant.num_results;
    // 1. Convert single query to Vec<String>
    let embedding = ollama.embed(vec![query.to_string()]).await?;

    // 2. Extract first vector from batch result
    let search_vector = embedding
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No embedding returned"))?;

    // 3. Pass Vec<f32> to Qdrant
    let search_points = SearchPointsBuilder::new(
        &config.qdrant.collection_name,
        search_vector,
        limit as u64
    ) .with_payload(true);
    let search_result: Vec<ScoredPoint> = client.search_points(search_points).await?.result;
    Ok(search_result.into_iter().map(|x| SearchResult::from(x)).collect() )
}
