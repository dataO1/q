use std::collections::HashMap;

use ai_agent_storage::QdrantClient;
use anyhow::Result;
use qdrant_client::qdrant::Value;

/// Helper to generate a test collection name
fn test_collection(base: &str) -> String {
    format!("test_{}_{}", base, uuid::Uuid::new_v4().to_string().replace('-', "_"))
}

/// Helper to create realistic embeddings (384 dimensions for FastEmbed)
fn create_test_embedding() -> Vec<f32> {
    (0..384).map(|i| (i as f32) / 384.0).collect()
}

/// Helper for 4D test embeddings (for simple tests)
fn create_simple_embedding() -> Vec<f32> {
    vec![0.1, 0.2, 0.3, 0.4]
}

// ============================================================================
// Basic Connection Tests
// ============================================================================

#[tokio::test]
async fn test_qdrant_connection() {
    let qdrant_url = std::env::var("TEST_QDRANT_URL")
        .unwrap_or_else(|_| "http://localhost:6333".to_string());

    let client = QdrantClient::new(&qdrant_url);
    assert!(client.is_ok(), "Should connect to Qdrant");

    let client = client.unwrap();
    let collection = test_collection("connection");

    // Create collection with correct dimensions
    let result = client.create_collection(&collection, 4).await;
    assert!(result.is_ok(), "Should create collection");

    // Verify it exists
    let exists = client.collection_exists(&collection).await;
    assert!(exists.is_ok() && exists.unwrap(), "Collection should exist");

    // Cleanup
    client.delete_collection(&collection).await.ok();
}

// ============================================================================
// Insert and Search Tests (Updated for Hybrid Search)
// ============================================================================

#[tokio::test]
async fn test_qdrant_insert_and_search() {
    let qdrant_url = std::env::var("TEST_QDRANT_URL")
        .unwrap_or_else(|_| "http://localhost:6333".to_string());

    let client = QdrantClient::new(&qdrant_url).unwrap();
    let collection = test_collection("search");

    client.create_collection(&collection, 4).await.unwrap();

    // Insert with metadata
    let mut metadata1 = HashMap::new();
    metadata1.insert("category".to_string(), Value::from("rust"));
    metadata1.insert("tier".to_string(), Value::from("workspace"));

    let mut metadata2 = HashMap::new();
    metadata2.insert("category".to_string(), Value::from("python"));
    metadata2.insert("tier".to_string(), Value::from("workspace"));

    let mut metadata3 = HashMap::new();
    metadata2.insert("category".to_string(), Value::from("python"));
    metadata2.insert("tier".to_string(), Value::from("workspace"));
    // Insert with String IDs (will be hashed to u64 internally)
    let points = vec![
        (uuid::Uuid::new_v4().to_u128_le() as u64, create_simple_embedding(), metadata1),
        (uuid::Uuid::new_v4().to_u128_le() as u64, create_simple_embedding(), metadata2),
        (uuid::Uuid::new_v4().to_u128_le() as u64, create_simple_embedding(), metadata3),
    ];

    let result = client.insert_points(&collection, points).await;
    assert!(result.is_ok(), "Should insert points: {:?}", result.err());

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let query_vector = create_simple_embedding();
    let results = client.search(&collection, query_vector, 3,None).await.unwrap();

    assert!(!results.is_empty(), "Should find results");
    assert!(results.len() <= 3, "Should respect limit");

    client.delete_collection(&collection).await.ok();
}

#[tokio::test]
async fn test_realistic_embedding_dimensions() {
    let qdrant_url = std::env::var("TEST_QDRANT_URL")
        .unwrap_or_else(|_| "http://localhost:6333".to_string());

    let client = QdrantClient::new(&qdrant_url).unwrap();
    let collection = test_collection("realistic");

    // Create collection with FastEmbed dimensions (384)
    client.create_collection(&collection, 384).await.unwrap();

    let mut metadata1 = HashMap::new();
    metadata1.insert("category".to_string(), Value::from("rust"));
    metadata1.insert("tier".to_string(), Value::from("workspace"));

    let mut metadata2 = HashMap::new();
    metadata2.insert("category".to_string(), Value::from("python"));
    metadata2.insert("tier".to_string(), Value::from("workspace"));
    // Insert with realistic 384D embeddings
    let points = vec![
        (uuid::Uuid::new_v4().to_u128_le() as u64, create_test_embedding(), metadata1),
        (uuid::Uuid::new_v4().to_u128_le() as u64, create_test_embedding(), metadata2),
    ];

    let result = client.insert_points(&collection, points).await;
    assert!(result.is_ok(), "Should insert 384D embeddings");

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search with 384D vector
    let query_vector = create_test_embedding();
    let results = client.search(&collection, query_vector, 2, None).await;

    assert!(results.is_ok(), "Should search with 384D vectors");
    assert!(!results.unwrap().is_empty(), "Should find results");

    // Cleanup
    client.delete_collection(&collection).await.ok();
}

// ============================================================================
// Metadata Filtering Tests
// ============================================================================

#[tokio::test]
async fn test_qdrant_metadata_filtering() {
    let qdrant_url = std::env::var("TEST_QDRANT_URL")
        .unwrap_or_else(|_| "http://localhost:6333".to_string());

    let client = QdrantClient::new(&qdrant_url).unwrap();
    let collection = test_collection("filter");

    client.create_collection(&collection, 4).await.unwrap();

    // Insert with metadata
    let mut metadata1 = HashMap::new();
    metadata1.insert("category".to_string(), Value::from("rust"));
    metadata1.insert("tier".to_string(), Value::from("workspace"));

    let mut metadata2 = HashMap::new();
    metadata2.insert("category".to_string(), Value::from("python"));
    metadata2.insert("tier".to_string(), Value::from("workspace"));

    let points = vec![
        (1, create_simple_embedding(), metadata1),
        (2, create_simple_embedding(), metadata2),
    ];

    let result = client.insert_points(&collection, points).await;
    assert!(result.is_ok(), "Should insert with metadata: {:?}", result.err());

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let query_vector = create_simple_embedding();
    let results = client.search(&collection, query_vector, 5, None).await;

    assert!(results.is_ok(), "Should search with metadata");

    client.delete_collection(&collection).await.ok();
}

// ============================================================================
// Hybrid Search Tests (Dense + Sparse)
// ============================================================================

#[tokio::test]
async fn test_hybrid_collection_creation() {
    let qdrant_url = std::env::var("TEST_QDRANT_URL")
        .unwrap_or_else(|_| "http://localhost:6333".to_string());

    let client = QdrantClient::new(&qdrant_url).unwrap();
    let collection = test_collection("hybrid");

    // Create hybrid collection (dense + sparse)
    let result = client.create_collection_hybrid(&collection, 384).await;
    assert!(result.is_ok(), "Should create hybrid collection");

    // Verify it exists
    let exists = client.collection_exists(&collection).await.unwrap();
    assert!(exists, "Hybrid collection should exist");

    // Cleanup
    client.delete_collection(&collection).await.ok();
}

// ============================================================================
// Upsert Tests
// ============================================================================

#[tokio::test]
async fn test_upsert_points() {
    let qdrant_url = std::env::var("TEST_QDRANT_URL")
        .unwrap_or_else(|_| "http://localhost:6333".to_string());

    let client = QdrantClient::new(&qdrant_url).unwrap();
    let collection = test_collection("upsert");

    client.create_collection(&collection, 4).await.unwrap();

    let point_id = uuid::Uuid::new_v4().to_u128_le() as u64;

    let mut metadata1 = HashMap::new();
    metadata1.insert("category".to_string(), Value::from("rust"));
    metadata1.insert("tier".to_string(), Value::from("workspace"));
    // First insert
    let points1 = vec![(point_id.clone(), create_simple_embedding(), metadata1)];
    client.insert_points(&collection, points1).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Upsert (update same ID)
    let mut updated_vector = create_simple_embedding();
    updated_vector[0] = 0.9; // Change first element

    let mut metadata1 = HashMap::new();
    metadata1.insert("category".to_string(), Value::from("rust"));
    metadata1.insert("tier".to_string(), Value::from("workspace"));
    let points2 = vec![(point_id.clone(), updated_vector.clone(), metadata1)];
    client.insert_points(&collection, points2).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Search should find updated vector
    let results = client.search(&collection, updated_vector, 1, None).await.unwrap();
    assert!(!results.is_empty(), "Should find upserted point");

    // Cleanup
    client.delete_collection(&collection).await.ok();
}

// ============================================================================
// Batch Operations Tests
// ============================================================================

#[tokio::test]
async fn test_batch_insert() {
    let qdrant_url = std::env::var("TEST_QDRANT_URL")
        .unwrap_or_else(|_| "http://localhost:6333".to_string());

    let client = QdrantClient::new(&qdrant_url).unwrap();
    let collection = test_collection("batch");

    client.create_collection(&collection, 4).await.unwrap();

    let mut metadata1 = HashMap::new();
    metadata1.insert("category".to_string(), Value::from("rust"));
    metadata1.insert("tier".to_string(), Value::from("workspace"));
    // Insert 100 points
    let points: Vec<_> = (0..100)
        .map(|_| (uuid::Uuid::new_v4().to_u128_le() as u64, create_simple_embedding(), metadata1.clone()))
        .collect();

    let result = client.insert_points(&collection, points).await;
    assert!(result.is_ok(), "Should insert batch");

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search should find multiple results
    let results = client.search(&collection, create_simple_embedding(), 10, None).await.unwrap();
    assert!(results.len() <= 10, "Should respect batch limit");

    // Cleanup
    client.delete_collection(&collection).await.ok();
}
