pub mod postgres;
pub mod qdrant;
pub mod redis;

pub use postgres::PostgresClient;
pub use qdrant::{QdrantClient, SearchResult};
pub use redis::RedisCache;

/// Initialize all storage backends
pub async fn initialize_storage(
    postgres_url: &str,
    qdrant_url: &str,
    redis_url: Option<&str>,
) -> anyhow::Result<(PostgresClient, QdrantClient, Option<RedisCache>)> {
    // Initialize PostgreSQL
    let postgres = PostgresClient::new(postgres_url).await?;
    postgres.run_migrations().await?;

    // Initialize Qdrant
    let qdrant = QdrantClient::new(qdrant_url)?;

    // Initialize Redis if URL provided
    let redis = if let Some(url) = redis_url {
        Some(RedisCache::new(url).await?)
    } else {
        None
    };

    tracing::info!("All storage backends initialized successfully");
    Ok((postgres, qdrant, redis))
}
