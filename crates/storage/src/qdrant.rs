use anyhow::{Context, Result};
use swiftide::integrations::qdrant::qdrant_client::qdrant::HealthCheckReply;
use swiftide::integrations::qdrant::{qdrant_client, Qdrant as SwiftideQdrant};
use swiftide::indexing::EmbeddedField;

/// Minimalist Qdrant client - Pure Swiftide wrapper
///
/// Swiftide handles:
/// - Auto collection creation on first write
/// - Hybrid search (dense + sparse vectors)
/// - Query pipeline with retrieve/rerank
///
/// This wrapper provides a simplified interface for:
/// 1. Indexing: Get client for pipeline
/// 2. Querying: Will use Swiftide query pipeline (in RAG crate)
pub struct QdrantClient {
    url: String,
}

impl QdrantClient {
    /// Create new Qdrant client
    pub fn new(url: &str) -> Result<Self> {
        Ok(Self {
            url: url.to_string(),
        })
    }

    /// Get Swiftide Qdrant client for indexing pipelines
    ///
    /// Auto-creates hybrid search collection on first write with:
    /// - Dense vectors (384D FastEmbed)
    /// - Sparse vectors (SPLADE)
    /// - Cosine distance
    ///
    /// Example:
    /// ```
    /// let qdrant = client.indexing_client("workspace_code")?;
    /// pipeline.then_store_with(qdrant).run().await?;
    /// ```
    pub fn indexing_client(&self, collection: &str) -> Result<SwiftideQdrant> {
        SwiftideQdrant::try_from_url(&self.url)?
            .vector_size(384)  // FastEmbed default
            .batch_size(50)
            .with_vector(EmbeddedField::Combined)
            .with_sparse_vector(EmbeddedField::Combined)
            .collection_name(collection.to_string())
            .build()
            .context("Failed to build Swiftide Qdrant client")
    }

    /// Get Swiftide Qdrant client for query pipelines (RAG retrieval)
    ///
    /// Use this in your RAG crate query pipeline:
    /// ```
    /// let qdrant = client.query_client("workspace_code")?;
    ///
    /// query::Pipeline::default()
    ///     .then_transform_query(Embed::new(embedder))
    ///     .then_retrieve(qdrant)  // ← Swiftide handles search!
    ///     .then_rerank(reranker)
    ///     .query("How to use Swiftide?")
    ///     .await?;
    /// ```
    pub fn query_client(&self, collection: &str) -> Result<SwiftideQdrant> {
        // Same builder - Swiftide uses it for both indexing and querying
        self.indexing_client(collection)
    }


    pub async fn health_check(&self) -> anyhow::Result<HealthCheckReply> {
        use qdrant_client::Qdrant;

        let client = Qdrant::from_url(&self.url)
            .build()
            .context("Failed to connect to Qdrant")?;

        Ok(client.health_check().await?)
    }

    /// Get the URL (for passing to other Swiftide components)
    pub fn url(&self) -> &str {
        &self.url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = QdrantClient::new("http://localhost:6333");
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_indexing_client_build() {
        let client = QdrantClient::new("http://localhost:6333").unwrap();
        let result = client.indexing_client("test_collection");
        assert!(result.is_ok());
    }
}
