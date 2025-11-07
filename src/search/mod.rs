use crate::Config;
use anyhow::{Context, Result};
use qdrant_client::qdrant::SearchPointsBuilder;
use serde::{Deserialize, Serialize};
use swiftide::integrations::ollama::Ollama;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
    pub snippet: String,
}

pub async fn search(
    query: &str,
    config: &Config,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    // Connect to Qdrant
    let qdrant = qdrant_client::Qdrant::from_url(&config.qdrant.url)
        .build()
        .context("Failed to connect to Qdrant")?;

    // Embed query using Ollama
    let ollama = Ollama::builder()
        .default_embed_model(&config.ollama.embedding_model)
        .build()?;

    // Get embedding for query
    let embedding = ollama.embed(query).await
        .context("Failed to embed query")?;

    // Search in Qdrant
    let search_result = qdrant
        .search_points(
            SearchPointsBuilder::new(&config.qdrant.collection_name, embedding, limit as u64)
                .with_payload(true)
        )
        .await
        .context("Failed to search")?;

    // Convert results
    let results = search_result
        .result
        .into_iter()
        .map(|point| {
            let payload = point.payload;
            let path = payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let snippet = payload
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .chars()
                .take(100)
                .collect::<String>();

            SearchResult {
                path,
                score: point.score,
                snippet,
            }
        })
        .collect();

    Ok(results)
}
