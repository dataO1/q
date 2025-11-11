use anyhow::{Context, Result, anyhow};
use qdrant_client::qdrant::r#match::MatchValue;
use swiftide::integrations::qdrant::{qdrant_client, Qdrant as SwiftideQdrant};
use swiftide::indexing::EmbeddedField;
use qdrant_client::qdrant::{Condition, Filter, SearchPointsBuilder};
use qdrant_client::Qdrant;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use ai_agent_common::{ContextFragment, ProjectScope};

/// Hybrid Qdrant client combining Swiftide for indexing and raw qdrant-client for filtered queries
pub struct QdrantClient {
    url: String,
    raw_client: Qdrant,
    embedder: TextEmbedding,
}

impl QdrantClient {
    /// Create new Qdrant client
    pub fn new(url: &str) -> anyhow::Result<Self> {
        let raw_client = Qdrant::from_url(url)
            .build()
            .context("Failed to connect to Qdrant")?;
        let options = InitOptions::new(EmbeddingModel::AllMiniLML6V2);
        let embedder = TextEmbedding::try_new(
            options
        ).context("Failed to initialize FastEmbed embedder")?;

        Ok(Self {
            url: url.to_string(),
            raw_client,
            embedder,
        })
    }

    /// Get Swiftide Qdrant client for indexing pipelines
    pub fn indexing_client(&self, collection: &str) -> Result<SwiftideQdrant> {
        SwiftideQdrant::try_from_url(&self.url)?
            .vector_size(384)
            .batch_size(50)
            .with_vector(EmbeddedField::Combined)
            .with_sparse_vector(EmbeddedField::Combined)
            .collection_name(collection)
            .build()
            .context("Failed to build Swiftide Qdrant client")
    }

    /// Query with metadata filtering using raw qdrant-client
    pub async fn query_with_filters(
        &self,
        collection: &str,
        query: &str,
        ctx: &ProjectScope,
        limit: u64,
    ) -> Result<Vec<ContextFragment>> {
        // Generate query embedding
        let query_embeddings = self.embedder
            .embed(vec![query.to_string()], None)
            .context("Failed to embed query")?;

        let query_vector = query_embeddings
            .first()
            .context("No embedding generated")?
            .clone();

        // Build metadata filter
        let filter = self.build_metadata_filter(&ctx)?;

        // Execute search with filter
        let search_result = self.raw_client
            .search_points(
                SearchPointsBuilder::new(collection, query_vector, limit)
                    .filter(filter)
                    .with_payload(true)
            )
            .await
            .context("Qdrant search failed")?;

        // Convert to ContextFragments
        Ok(search_result
            .result
            .into_iter()
            .map(|point| {
                let payload = point.payload;
                ContextFragment {
                    content: payload
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&"".to_string())
                        .to_string(),
                    summary: payload
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&"".to_string())
                        .to_string(),
                    source: payload
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&"".to_string())
                        .to_string(),
                    score: point.score,
                }
            })
            .collect())
    }

    /// Build Qdrant metadata filter from AgentContext
    fn build_metadata_filter(&self, ctx: &ProjectScope) -> Result<Filter> {
        use qdrant_client::qdrant::{condition::ConditionOneOf, FieldCondition, Match};

        let root = ctx.root.to_str()
            .ok_or(anyhow!("Invalid UTF-8 in path"))?
            .to_string();

        let mut must_conditions = vec![];

        // Project root exact match
        must_conditions.push(Condition {
            condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                // TODO: this assumes the project_root field in the db entry
                key: "project_root".to_string(),
                r#match: Some(Match::from(MatchValue::Text(root))),
                ..Default::default()
            })),
        });

        Ok(Filter {
            must: must_conditions,
            ..Default::default()
        })
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        self.raw_client
            .health_check()
            .await
            .context("Qdrant health check failed")?;
        Ok(())
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ai_agent_common::Language;

    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = QdrantClient::new("http://localhost:6333");
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_filter_building() {
        let client = QdrantClient::new("http://localhost:6333").unwrap();
        let ctx = ProjectScope {
            root: PathBuf::from("/workspace".to_string()),
            language_distribution: vec![(Language::Rust, 100f32)],
            current_file: Some(PathBuf::from("data.txt".to_string())),
        };

        let filter = client.build_metadata_filter(&ctx);
        assert!(filter.is_ok());
    }
}
