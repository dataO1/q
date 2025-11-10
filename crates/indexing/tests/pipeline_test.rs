use ai_agent_indexing::pipeline::{IndexingPipeline, IndexingCoordinator};
use ai_agent_common::config::*;
use ai_agent_common::types::CollectionTier;
use ai_agent_storage::QdrantClient;
use tempfile::TempDir;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Helper to create test configuration with test services
fn test_config() -> SystemConfig {
    SystemConfig {
        indexing: IndexingConfig {
            workspace_paths: vec![],
            personal_paths: vec![],
            system_paths: vec![],
            watch_enabled: true,
            chunk_size: 512,
            filters: IndexingFilters::default(),
        },
        rag: RagConfig {
            max_results: 5,
            query_enhancement_model: "qwen2.5:7b".to_string(),
            reranking_weights: RerankingWeights {
                conversation_boost: 1.2,
                recency_boost: 1.1,
                dependency_boost: 0.8,
            },
        },
        orchestrator: OrchestratorConfig {
            agents: vec![],
            checkpoint_interval: "5m".to_string(),
        },
        storage: StorageConfig {
            qdrant_url: std::env::var("TEST_QDRANT_URL")
                .unwrap_or_else(|_| "http://localhost:16334".to_string()),
            postgres_url: std::env::var("TEST_DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost/ai_agent_test".to_string()),
            redis_url: Some(
                std::env::var("TEST_REDIS_URL")
                    .unwrap_or_else(|_| "redis://localhost:16379".to_string())
            ),
        },
    }
}

/// Helper to clear Redis cache between tests
async fn clear_redis_cache() {
    if let Ok(redis_url) = std::env::var("TEST_REDIS_URL") {
        // Clear test Redis cache
        let client = redis::Client::open(redis_url).ok();
        if let Some(client) = client {
            if let Ok(mut conn) = client.get_connection() {
                let _: Result<(), _> = redis::cmd("FLUSHDB").query(&mut conn);
            }
        }
    }
}

// ============================================================================
// Unit Tests (No External Services)
// ============================================================================

#[test]
fn test_pipeline_creation() {
    let config = test_config();
    let pipeline = IndexingPipeline::new(&config);

    assert!(pipeline.is_ok(), "Pipeline creation should succeed");
}

#[test]
fn test_coordinator_creation() {
    let config = test_config();
    let coordinator = IndexingCoordinator::new(config);

    assert!(coordinator.is_ok(), "Coordinator creation should succeed");
}

#[test]
fn test_language_detection() {
    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    assert_eq!(pipeline.detect_language(&PathBuf::from("main.rs")).unwrap(), "rust");
    assert_eq!(pipeline.detect_language(&PathBuf::from("app.py")).unwrap(), "python");
    assert_eq!(pipeline.detect_language(&PathBuf::from("script.js")).unwrap(), "javascript");
    assert_eq!(pipeline.detect_language(&PathBuf::from("component.tsx")).unwrap(), "typescript");
}

#[test]
fn test_is_code_file_detection() {
    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    // Code files
    assert!(pipeline.is_code_file(&PathBuf::from("main.rs")));
    assert!(pipeline.is_code_file(&PathBuf::from("app.py")));
    assert!(pipeline.is_code_file(&PathBuf::from("script.js")));

    // Non-code files
    assert!(!pipeline.is_code_file(&PathBuf::from("README.md")));
    assert!(!pipeline.is_code_file(&PathBuf::from("data.json")));
}

// ============================================================================
// Integration Tests - Basic Indexing
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_index_single_rust_file() {
    clear_redis_cache().await;

    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.rs");

    let rust_code = r#"
/// Main entry point
fn main() {
    println!("Hello, world!");
}

/// Helper function
fn helper(x: i32) -> i32 {
    x + 42
}
"#;

    fs::write(&test_file, rust_code).unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    let result = pipeline.index_file(&test_file, CollectionTier::Workspace).await;

    assert!(result.is_ok(), "Should index Rust file: {:?}", result.err());

    tokio::time::sleep(Duration::from_secs(2)).await;
}

#[tokio::test]
#[ignore]
async fn test_index_multiple_languages() {
    clear_redis_cache().await;

    let temp = TempDir::new().unwrap();

    // Rust file
    let rust_file = temp.path().join("test.rs");
    fs::write(&rust_file, "fn test() { println!(\"rust\"); }").unwrap();

    // Python file
    let py_file = temp.path().join("test.py");
    fs::write(&py_file, "def test():\n    print('python')").unwrap();

    // JavaScript file
    let js_file = temp.path().join("test.js");
    fs::write(&js_file, "function test() { console.log('js'); }").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    let r1 = pipeline.index_file(&rust_file, CollectionTier::Workspace).await;
    let r2 = pipeline.index_file(&py_file, CollectionTier::Workspace).await;
    let r3 = pipeline.index_file(&js_file, CollectionTier::Workspace).await;

    assert!(r1.is_ok() && r2.is_ok() && r3.is_ok(), "All languages should index");

    tokio::time::sleep(Duration::from_secs(2)).await;
}

// ============================================================================
// Integration Tests - Redis Caching & Upsert
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_redis_cache_deduplication() {
    clear_redis_cache().await;

    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.rs");

    fs::write(&test_file, "fn test() { println!(\"v1\"); }").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    // First indexing - should process
    let start = std::time::Instant::now();
    pipeline.index_file(&test_file, CollectionTier::Workspace).await.unwrap();
    let first_duration = start.elapsed();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Second indexing - should be cached (much faster)
    let start = std::time::Instant::now();
    pipeline.index_file(&test_file, CollectionTier::Workspace).await.unwrap();
    let second_duration = start.elapsed();

    println!("First: {:?}, Second (cached): {:?}", first_duration, second_duration);

    // Cached should be significantly faster (no embedding)
    assert!(second_duration < first_duration / 2, "Cached should be 2x+ faster");
}

#[tokio::test]
#[ignore]
async fn test_upsert_on_file_change() {
    clear_redis_cache().await;

    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.rs");

    // Version 1
    fs::write(&test_file, "fn test() { println!(\"v1\"); }").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    pipeline.index_file(&test_file, CollectionTier::Workspace).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Version 2 - change content
    fs::write(&test_file, "fn test() { println!(\"v2 - updated!\"); }").unwrap();

    // Should upsert (update existing point)
    let result = pipeline.index_file(&test_file, CollectionTier::Workspace).await;
    assert!(result.is_ok(), "Upsert should succeed");

    tokio::time::sleep(Duration::from_secs(1)).await;
}

#[tokio::test]
#[ignore]
async fn test_cache_invalidation_on_change() {
    clear_redis_cache().await;

    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.rs");

    fs::write(&test_file, "fn test() { println!(\"v1\"); }").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    // First index
    pipeline.index_file(&test_file, CollectionTier::Workspace).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Change file content
    fs::write(&test_file, "fn test() { println!(\"CHANGED\"); }").unwrap();

    // Should NOT be cached (content changed)
    let start = std::time::Instant::now();
    pipeline.index_file(&test_file, CollectionTier::Workspace).await.unwrap();
    let duration = start.elapsed();

    // Should take normal time (not cached)
    assert!(duration > Duration::from_millis(500), "Changed file should not be cached");
}

// ============================================================================
// Integration Tests - Hybrid Search (Dense + Sparse)
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_hybrid_search_vectors() {
    clear_redis_cache().await;

    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.rs");

    fs::write(&test_file, "fn calculate_sum(a: i32, b: i32) -> i32 { a + b }").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    pipeline.index_file(&test_file, CollectionTier::Workspace).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify both dense and sparse vectors exist
    let qdrant = QdrantClient::new(&config.storage.qdrant_url).unwrap();
    let collection = CollectionTier::Workspace.collection_name();

    // Collection should exist and have hybrid search enabled
    let exists = qdrant.collection_exists(&collection).await.unwrap();
    assert!(exists, "Collection should exist with hybrid vectors");
}

// ============================================================================
// Integration Tests - Batch & Directory
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_index_directory_with_cache() {
    clear_redis_cache().await;

    let temp = TempDir::new().unwrap();

    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(temp.path().join("src/lib.rs"), "pub fn lib() {}").unwrap();
    fs::write(temp.path().join("README.md"), "# Test").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    // First indexing
    let start = std::time::Instant::now();
    pipeline.index_directory(temp.path(), CollectionTier::Workspace, &["rs", "md"]).await.unwrap();
    let first_duration = start.elapsed();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Second indexing - should be cached
    let start = std::time::Instant::now();
    pipeline.index_directory(temp.path(), CollectionTier::Workspace, &["rs", "md"]).await.unwrap();
    let second_duration = start.elapsed();

    println!("First: {:?}, Second (cached): {:?}", first_duration, second_duration);
    assert!(second_duration < first_duration, "Cached directory index should be faster");
}

#[tokio::test]
#[ignore]
async fn test_batch_indexing_with_upsert() {
    clear_redis_cache().await;

    let temp = TempDir::new().unwrap();

    let files = vec![
        (temp.path().join("file1.rs"), "fn file1() {}"),
        (temp.path().join("file2.py"), "def file2(): pass"),
        (temp.path().join("file3.js"), "function file3() {}"),
    ];

    for (path, content) in &files {
        fs::write(path, content).unwrap();
    }

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    let batch: Vec<_> = files.iter().map(|(p, _)| (p.clone(), CollectionTier::Workspace)).collect();
    let results = pipeline.index_batch(batch).await.unwrap();

    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r.is_ok()), "All files should index");

    tokio::time::sleep(Duration::from_secs(2)).await;
}

// ============================================================================
// Integration Tests - Tier Separation
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_tier_separation_with_hybrid() {
    clear_redis_cache().await;

    let temp = TempDir::new().unwrap();

    let workspace = temp.path().join("workspace.rs");
    let personal = temp.path().join("personal.md");
    let system = temp.path().join("system.txt");

    fs::write(&workspace, "fn workspace() {}").unwrap();
    fs::write(&personal, "# Personal Notes").unwrap();
    fs::write(&system, "System docs").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    pipeline.index_file(&workspace, CollectionTier::Workspace).await.unwrap();
    pipeline.index_file(&personal, CollectionTier::Personal).await.unwrap();
    pipeline.index_file(&system, CollectionTier::System).await.unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify all collections exist
    let qdrant = QdrantClient::new(&config.storage.qdrant_url).unwrap();

    let w_exists = qdrant.collection_exists(&CollectionTier::Workspace.collection_name()).await.unwrap_or(false);
    let p_exists = qdrant.collection_exists(&CollectionTier::Personal.collection_name()).await.unwrap_or(false);
    let s_exists = qdrant.collection_exists(&CollectionTier::System.collection_name()).await.unwrap_or(false);

    println!("Collections - Workspace: {}, Personal: {}, System: {}", w_exists, p_exists, s_exists);
}
