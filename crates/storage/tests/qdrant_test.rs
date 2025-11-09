use ai_agent_storage::QdrantClient;
use qdrant_client::qdrant::Value;
use std::collections::HashMap;
use uuid::Uuid;

fn get_test_qdrant_url() -> String {
    std::env::var("TEST_QDRANT_URL")
        .expect("TEST_QDRANT_URL must be set (use docker-compose.test.yml)")
}

// Generate unique collection name
fn test_collection_name(suffix: &str) -> String {
    format!("test_{}_{}", suffix, Uuid::new_v4().to_string().replace('-', "_"))
}

#[tokio::test]
#[ignore]
async fn test_qdrant_connection() {
    // Add timeout and retry logic
    let client = QdrantClient::new(&get_test_qdrant_url())
        .expect("Failed to create Qdrant client");

    let collection = test_collection_name("connection");

    // Give Qdrant time to be ready
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    client.create_collection(&collection, 768).await
        .expect("Failed to create collection");

    let exists = client.collection_exists(&collection).await.unwrap();
    assert!(exists);
}

#[tokio::test]
#[ignore]
async fn test_qdrant_insert_and_search() {
    let client = QdrantClient::new(&get_test_qdrant_url()).unwrap();
    let collection = test_collection_name("search");

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    client.ensure_collection(&collection, 4).await.unwrap();

    // Insert test points
    let mut payload = HashMap::new();
    payload.insert("text".to_string(), Value::from("Hello world"));
    payload.insert("language".to_string(), Value::from("rust"));

    client.insert_point(&collection, 1, vec![0.1, 0.2, 0.3, 0.4], payload)
        .await
        .unwrap();

    // Give time for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search
    let results = client
        .search(&collection, vec![0.1, 0.2, 0.3, 0.4], 5, None)
        .await
        .unwrap();

    assert!(!results.is_empty());
    assert_eq!(results[0].id, 1);
}

#[tokio::test]
#[ignore]
async fn test_qdrant_metadata_filtering() {
    let client = QdrantClient::new(&get_test_qdrant_url()).unwrap();
    let collection = test_collection_name("filter");

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    client.ensure_collection(&collection, 4).await.unwrap();

    // Insert points with metadata
    let mut payload1 = HashMap::new();
    payload1.insert("language".to_string(), Value::from("rust"));
    payload1.insert("file_type".to_string(), Value::from("rs"));

    let mut payload2 = HashMap::new();
    payload2.insert("language".to_string(), Value::from("python"));
    payload2.insert("file_type".to_string(), Value::from("py"));

    client.insert_point(&collection, 1, vec![0.1, 0.2, 0.3, 0.4], payload1)
        .await
        .unwrap();
    client.insert_point(&collection, 2, vec![0.5, 0.6, 0.7, 0.8], payload2)
        .await
        .unwrap();

    // Give time for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search with filter
    let results = client
        .search_with_metadata(
            &collection,
            vec![0.1, 0.2, 0.3, 0.4],
            5,
            None,
            Some("rust"),
            None,
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 1);
}
