pub mod postgres;
pub mod qdrant;
pub mod redis;

use ai_agent_common::llm::EmbeddingClient;
pub use postgres::PostgresClient;
pub use qdrant::QdrantClient;
pub use redis::RedisCache;

/// Initialize all storage backends
pub async fn initialize_storage<'a>(
    postgres_url: &str,
    qdrant_url: &str,
    redis_url: Option<&str>,
    embedding_client:  &'a EmbeddingClient,
) -> anyhow::Result<(PostgresClient, QdrantClient<'a>, Option<RedisCache>)> {
    // Initialize PostgreSQL
    let postgres = PostgresClient::new(postgres_url).await?;
    postgres.run_migrations().await?;

    // Initialize Qdrant
    let qdrant = QdrantClient::<'a>::new(qdrant_url, embedding_client)?;

    // Initialize Redis if URL provided
    let redis = if let Some(url) = redis_url {
        Some(RedisCache::new(url).await?)
    } else {
        None
    };

    tracing::info!("All storage backends initialized successfully");
    Ok((postgres, qdrant, redis))
}
