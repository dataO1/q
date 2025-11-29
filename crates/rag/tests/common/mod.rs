//! Common test utilities for RAG testing

pub mod mock_web_server;

use ai_agent_common::{SystemConfig, llm::EmbeddingClient, ProjectScope};
use ai_agent_storage::{QdrantClient, RedisCache};
use ai_agent_indexing::IndexingPipeline;
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

/// Create a test system config by loading from config.dev.toml
pub fn create_test_config() -> SystemConfig {
    // Try to find the config file relative to the project root
    let config_paths = [
        "config.dev.toml",                    // If running from project root
        "../../../config.dev.toml",          // If running from crates/rag/tests
        "../../config.dev.toml",             // If running from different context
        "../config.dev.toml",                // Alternative relative path
    ];
    
    for path in &config_paths {
        if std::path::Path::new(path).exists() {
            return SystemConfig::from_file(path)
                .unwrap_or_else(|e| panic!("Failed to load config from {}: {}", path, e));
        }
    }
    
    panic!("Could not find config.dev.toml in any of the expected locations: {:?}", config_paths);
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
        vec![("Rust".to_string(), 1.0)]
    )
}

/// Create test embeddings (384 dimensions for all-minilm model)
pub fn create_test_embedding() -> Vec<f32> {
    (0..384).map(|i| (i as f32) / 384.0).collect()
}

/// Setup test collections by checking if they exist and creating test data if needed
pub async fn setup_test_collections() -> Result<()> {
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    
    // Check if workspace collection exists
    let workspace_exists = check_collection_exists(&qdrant, "workspace").await;
    let personal_exists = check_collection_exists(&qdrant, "personal").await;
    
    if !workspace_exists || !personal_exists {
        println!("Creating test collections and indexing test data...");
        
        // Create indexing pipeline
        let embedder = create_test_embedding_client()?;
        let indexing_pipeline = IndexingPipeline::new(&config, embedder.clone())?;
        
        // Index test workspace data
        if !workspace_exists && !config.indexing.workspace_paths.is_empty() {
            use ai_agent_common::CollectionTier;
            for workspace_path in &config.indexing.workspace_paths {
                println!("Indexing workspace directory: {}", workspace_path.display());
                indexing_pipeline.index_directory(workspace_path, CollectionTier::Workspace, None).await
                    .unwrap_or_else(|e| println!("Warning: Failed to index {}: {}", workspace_path.display(), e));
            }
        }
        
        // Index test personal data  
        if !personal_exists && !config.indexing.personal_paths.is_empty() {
            use ai_agent_common::CollectionTier;
            for personal_path in &config.indexing.personal_paths {
                println!("Indexing personal directory: {}", personal_path.display());
                indexing_pipeline.index_directory(personal_path, CollectionTier::Personal, None).await
                    .unwrap_or_else(|e| println!("Warning: Failed to index {}: {}", personal_path.display(), e));
            }
        }
        
        println!("Test data indexing completed");
    } else {
        println!("Test collections already exist, skipping indexing");
    }
    
    Ok(())
}

/// Check if a collection exists in Qdrant by trying to query it
async fn check_collection_exists(qdrant: &QdrantClient, collection_name: &str) -> bool {
    use ai_agent_common::CollectionTier;
    
    // Try to query the collection - if it fails, collection doesn't exist
    let tier = match collection_name {
        "workspace" => CollectionTier::Workspace,
        "personal" => CollectionTier::Personal,
        _ => return false,
    };
    
    let test_project_scope = create_test_project_scope();
    
    match qdrant.query_collections(
        vec![(tier, "test query".to_string())],
        &test_project_scope,
        Some(1),
    ).await {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Cleanup test collections in Qdrant (for future use)
/// Note: Currently disabled as per requirements - leaving test data for reuse
pub async fn cleanup_test_collections(_qdrant: &QdrantClient, collections: &[String]) {
    // Intentionally disabled - keeping test data for reuse as requested
    println!("Test cleanup disabled - keeping collections for reuse: {:?}", collections);
}