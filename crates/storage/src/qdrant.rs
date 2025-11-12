use anyhow::{Context, Result, anyhow};
use qdrant_client::qdrant::r#match::MatchValue;
use qdrant_client::qdrant::vector_output::Vector;
use qdrant_client::qdrant::vectors_output::VectorsOptions;
use swiftide::integrations::qdrant::{qdrant_client, Qdrant as SwiftideQdrant};
use swiftide::indexing::EmbeddedField;
use qdrant_client::qdrant::{Condition, Filter, SearchPointsBuilder};
use qdrant_client::Qdrant;
use fastembed::{EmbeddingModel, InitOptions, SparseEmbedding, TextEmbedding};
use ai_agent_common::{CollectionTier, ContextFragment, ProjectScope};

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

    /// Query multiple collections (tiers) asynchronously and aggregate results
    pub async fn query_collections(
        &self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &ProjectScope,
    ) -> Result<Vec<(ContextFragment, SparseEmbedding)>> {
        let mut all_results = Vec::new();

        // For each tier + query tuple, run query_with_filters with appropriate collection name or parameters
        for (tier, query) in queries {
            // Derive collection name or namespace from the tier
            let collection_name = tier.collection_name();

            // Run query with filters for this collection and query text
            let mut results = self.query_with_filters(&collection_name, &query, project_scope).await?;

            all_results.append(&mut results);
        }

        Ok(all_results)
    }

    /// Query with metadata filtering using raw qdrant-client
    pub async fn query_with_filters(
        &self,
        collection: &str,
        query: &str,
        project_scope: &ProjectScope,
    ) -> Result<Vec<(ContextFragment, SparseEmbedding)>> {
        let query_embedding = self.embedder
            .embed(vec![query.to_string()], None)
            .context("Failed to embed query")?;

        let query_vector = query_embedding
            .first()
            .context("No embedding generated")?
            .clone();

        // Build your filter for metadata (project_scope), omitted here for brevity
        let filter = self.build_metadata_filter(project_scope)?;

        let search_result = self.raw_client
            .search_points(
                SearchPointsBuilder::new(collection, query_vector, 10)
                    .filter(filter)
                    .with_vectors(true)  // request vector field return
                    .with_payload(true)
            )
            .await?;

        let mut results = Vec::new();

        for point in search_result.result {
            // Extract payload data to ContextFragment
            let payload = &point.payload;

            let fragment = ContextFragment {
                content: payload.get("content").and_then(|v| v.as_str()).unwrap_or(&"".to_string()).to_string(),
                summary: payload.get("summary").and_then(|v| v.as_str()).unwrap_or(&"".to_string()).to_string(),
                source: payload.get("source").and_then(|v| v.as_str()).unwrap_or(&"unknown".to_string()).to_string(),
                score: ( point.score * 100f32 ) as usize,
            };

            // Extract embedding vector from raw vector field
            // Assuming vector is a repeated f32 field (dense)
            let emb_vec: Option<SparseEmbedding> = match &point.vectors {
                Some(v) => match &v.vectors_options {
                    Some(vectors_options) => {
                        match vectors_options {
                            // Match on your sparse vector variant, e.g., `VectorsOptions::Sparse`
                            // Adjust enum variant name according to actual client version
                            VectorsOptions::Vectors(named_vectors) => {
                                let sparse_vector = named_vectors.vectors.get("Combined_sparse");
                                let indices = sparse_vector.unwrap().indices.clone().unwrap().data.into_iter().map(|x| x as usize).collect();
                                let values = match &sparse_vector.clone().unwrap().vector{
                                    Some(Vector::Sparse(vec))=>Some(vec.values.clone()),
                                    _ => None
                                }.unwrap_or(vec![]);
                                Some(SparseEmbedding {
                                    indices,
                                    values
                                })
                            }
                            _ => None,
                        }
                    }
                    None => None,
                },  // raw dense vector usually in .f
                None => None,
            };
            let embedding = SparseEmbedding::from(emb_vec.unwrap());

            results.push((fragment, embedding));
        }

        Ok(results)
    }

    /// Build Qdrant metadata filter from AgentContext
    fn build_metadata_filter(&self, ctx: &ProjectScope) -> Result<Filter> {
        use qdrant_client::qdrant::{condition::ConditionOneOf, FieldCondition, Match};


        let mut must_conditions = vec![];

        // Project root exact match
        must_conditions.push(Condition {
            condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                // TODO: this assumes the project_root field in the db entry
                key: "project_root".to_string(),
                r#match: Some(Match::from(MatchValue::Text(ctx.root.clone()))),
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
            root: "/workspace".to_string(),
            language_distribution: vec![(Language::Rust, 100f32)],
            current_file: Some(PathBuf::from("data.txt".to_string())),
        };

        let filter = client.build_metadata_filter(&ctx);
        assert!(filter.is_ok());
    }
}
