use ai_agent_common::llm::EmbeddingClient;
use anyhow::{Context, Result};
use qdrant_client::qdrant::r#match::MatchValue;
use swiftide::integrations::qdrant::{qdrant_client, Qdrant as SwiftideQdrant};
use swiftide::indexing::EmbeddedField;
use qdrant_client::qdrant::{Condition, Filter, Fusion, PrefetchQueryBuilder, Query, QueryPointsBuilder, VectorInput};
use qdrant_client::Qdrant;
use swiftide::indexing::EmbeddingModel;
use ai_agent_common::{CollectionTier, ContextFragment, ProjectScope};
use qdrant_client::qdrant::{condition::ConditionOneOf, FieldCondition, Match};
use swiftide::traits::SparseEmbeddingModel;
use swiftide::{SparseEmbedding, SparseEmbeddings};

/// Hybrid Qdrant client combining Swiftide for indexing and raw qdrant-client for filtered queries
#[derive(Clone)]
pub struct QdrantClient<'a> {
    url: String,
    raw_client: Qdrant,
    embedder: &'a EmbeddingClient,
}

impl<'a> QdrantClient<'a> {
    /// Create new Qdrant client
    pub fn new(url: &str, embedder: &'a EmbeddingClient) -> anyhow::Result<Self> {
        let raw_client = Qdrant::from_url(url)
            .build()
            .context("Failed to connect to Qdrant")?;

        Ok(Self {
            url: url.to_string(),
            raw_client,
            embedder,
        })
    }

    /// Get Swiftide Qdrant client for indexing pipelines
    pub fn indexing_client(&self, collection: &str) -> Result<SwiftideQdrant> {
        SwiftideQdrant::try_from_url(&self.url)?
            .vector_size(self.embedder.vector_size_dense)
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
        limit: Option<u64>,
    ) -> Result<Vec<ContextFragment>> {
        let mut all_results = Vec::new();

        // For each tier + query tuple, run query_with_filters with appropriate collection name or parameters
        for (tier, query) in queries {
            // Derive collection name or namespace from the tier
            let collection_name = tier.to_string();

            // Run query with filters for this collection and query text
            let mut results = self.hybrid_query_with_filters(&collection_name, &query, project_scope, limit).await?;

            all_results.append(&mut results);
        }

        Ok(all_results)
    }

    /// Query with metadata filtering using raw qdrant-client
    pub async fn hybrid_query_with_filters(
        &self,
        collection: &str,
        query: &str,
        project_scope: &ProjectScope,
        limit: Option<u64>,
    ) -> Result<Vec<ContextFragment>> {
        let sparse_embedding: SparseEmbedding = self.embedder.embedder_sparse
            .sparse_embed(vec![query.to_string()]).await?.first().context("Failed to generate sparse query embedding")?.clone();
        let dense_embedding = self.embedder.embedder_dense
            .embed(vec![query.to_string()]).await?.first().context("Failed to generate dense query embedding")?.clone();
        // Build your filter for metadata (project_scope), omitted here for brevity
        let filter = self.build_metadata_filter(project_scope)?;

        let query = QueryPointsBuilder::new(collection)
        .add_prefetch(
            PrefetchQueryBuilder::default()
                .using("Combined_sparse")
                .filter(filter.clone())
                .query(Query::new_nearest(VectorInput::new_sparse(sparse_embedding.indices,sparse_embedding.values)))  // Dense branch
                .limit(50u64)
                .build()
        )
        .add_prefetch(
            PrefetchQueryBuilder::default()
                .using("Combined")
                .filter(filter)
                .query(Query::new_nearest(VectorInput::new_dense(dense_embedding.clone())))  // Dense branch
                .limit(30u64)
                .score_threshold(0.72)
                .build()
        )
            .query(Query::new_fusion(Fusion::Rrf))
            // .using("Combined_sparse")
            // .using("Combined")
            .with_payload(true)
            .limit(limit.unwrap_or(10));
        let q = query.build();
        let search_result = self.raw_client.query(q).await?;

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
            results.push(fragment);
        }

        Ok(results)
    }

    /// Build Qdrant metadata filter from AgentContext
    fn build_metadata_filter(&self, ctx: &ProjectScope) -> Result<Filter> {
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
