//! Common test utilities for RAG testing

pub mod mock_web_server;

use ai_agent_common::{SystemConfig, llm::EmbeddingClient, ProjectScope, Language};
use ai_agent_storage::{QdrantClient, RedisCache};
use anyhow::Result;
use std::sync::Arc;
use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize logging for tests
pub fn init_test_logging() {
    INIT.call_once(|| {
        let _ = env_logger::builder()
            .is_test(true)
            .try_init();
    });
}

/// Create a test system config with web crawler enabled
pub fn create_test_config() -> SystemConfig {
    let mut config = SystemConfig::default();
    
    // Set test-friendly values
    config.storage.qdrant_url = "http://localhost:16334".to_string();
    config.storage.redis_url = Some("redis://localhost:16379".to_string());
    config.embedding.vector_size = 384;
    config.embedding.dense_model = "all-minilm:l6-v2".to_string();
    
    // Enable web crawler with test settings
    config.rag.web_crawler.enabled = true;
    config.rag.web_crawler.max_urls_per_query = 3;
    config.rag.web_crawler.request_timeout_secs = 10;
    config.rag.web_crawler.content_cache_ttl_secs = 300;
    config.rag.web_crawler.query_cache_ttl_secs = 60;
    config.rag.web_crawler.chunk_size = 512;
    config.rag.web_crawler.chunk_overlap = 50;
    config.rag.web_crawler.user_agent = "RAGTestAgent/1.0".to_string();
    config.rag.web_crawler.respect_robots_txt = false;
    config.rag.web_crawler.web_content_collection = "test_web_content".to_string();
    config.rag.web_crawler.web_query_cache_collection = "test_web_query_cache".to_string();
    config.rag.web_crawler.content_cache_prefix = "test_web_content:".to_string();
    config.rag.web_crawler.query_cache_prefix = "test_web_query:".to_string();
    
    config
}

/// Create test embedding client
pub fn create_test_embedding_client() -> Result<Arc<EmbeddingClient>> {
    Ok(Arc::new(EmbeddingClient::new(
        &"all-minilm:l6-v2".to_string(),
        384,
    )?))
}

/// Create test Qdrant client
pub fn create_test_qdrant_client() -> Result<Arc<QdrantClient>> {
    let embedder = create_test_embedding_client()?;
    Ok(Arc::new(QdrantClient::new("http://localhost:16334", embedder)?))
}

/// Create test Redis client
pub async fn create_test_redis_client() -> Result<Arc<RedisCache>> {
    Ok(Arc::new(RedisCache::new("redis://localhost:16379").await?))
}

/// Generate a unique test collection name
pub fn test_collection_name(prefix: &str) -> String {
    format!("test_{}_{}", prefix, uuid::Uuid::new_v4().to_string().replace('-', "_"))
}

/// Create a test project scope
pub fn create_test_project_scope() -> ProjectScope {
    ProjectScope::new(
        "/test/project".to_string(),
        Some(std::path::PathBuf::from("/test/project/src/main.rs")),
        vec![(Language::Rust, 1.0)]
    )
}

/// Create test embeddings (384 dimensions for all-minilm model)
pub fn create_test_embedding() -> Vec<f32> {
    (0..384).map(|i| (i as f32) / 384.0).collect()
}

/// Cleanup test collections in Qdrant
pub async fn cleanup_test_collections(qdrant: &QdrantClient, collections: &[String]) {
    for collection in collections {
        let _ = qdrant.delete_collection(collection).await;
    }
}