use ai_agent_storage::RedisCache;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn get_test_redis_url() -> String {
    std::env::var("TEST_REDIS_URL")
        .expect("TEST_REDIS_URL must be set (use docker-compose.test.yml)")
}

// Generate unique key prefix for this test run
fn test_key_prefix() -> String {
    format!("test:{}:", Uuid::new_v4())
}

// Helper to create test key with unique prefix
fn test_key(name: &str, prefix: &str) -> String {
    format!("{}{}", prefix, name)
}

// Cleanup: Delete all test keys
async fn cleanup_redis_keys(cache: &RedisCache, prefix: &str) {
    let pattern = format!("{}*", prefix);
    if let Ok(keys) = cache.keys(&pattern).await {
        for key in keys {
            let _ = cache.delete(&key).await;
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_redis_connection() {
    let cache = RedisCache::new(&get_test_redis_url())
        .await
        .expect("Failed to connect to test Redis");

    cache.ping().await.expect("Ping failed");
}

#[tokio::test]
#[ignore]
async fn test_redis_set_and_get() {
    let cache = RedisCache::new(&get_test_redis_url()).await.unwrap();
    let prefix = test_key_prefix();
    let key = test_key("test_key", &prefix);

    cache.set(&key, "test_value").await.unwrap();

    let value = cache.get(&key).await.unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    cleanup_redis_keys(&cache, &prefix).await;
}

#[tokio::test]
#[ignore]
async fn test_redis_expiration() {
    let cache = RedisCache::new(&get_test_redis_url()).await.unwrap();
    let prefix = test_key_prefix();
    let key = test_key("expiring_key", &prefix);

    cache.set_ex(&key, "value", 1).await.unwrap();

    let value = cache.get(&key).await.unwrap();
    assert!(value.is_some());

    // Wait for expiration
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let value = cache.get(&key).await.unwrap();
    assert!(value.is_none());

    cleanup_redis_keys(&cache, &prefix).await;
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct TestStruct {
    name: String,
    count: i32,
}

#[tokio::test]
#[ignore]
async fn test_redis_json() {
    let cache = RedisCache::new(&get_test_redis_url()).await.unwrap();
    let prefix = test_key_prefix();
    let key = test_key("json_key", &prefix);

    let test_data = TestStruct {
        name: "test".to_string(),
        count: 42,
    };

    cache.set_json(&key, &test_data).await.unwrap();

    let retrieved: Option<TestStruct> = cache.get_json(&key).await.unwrap();
    assert_eq!(retrieved, Some(test_data));

    cleanup_redis_keys(&cache, &prefix).await;
}

#[tokio::test]
#[ignore]
async fn test_redis_cache_or_compute() {
    let cache = RedisCache::new(&get_test_redis_url()).await.unwrap();
    let prefix = test_key_prefix();
    let key = test_key("compute_test", &prefix);

    // First call should compute
    let value1 = cache
        .cache_or_compute(&key, 60, || async { Ok("computed_value".to_string()) })
        .await
        .unwrap();

    assert_eq!(value1, "computed_value");

    // Second call should use cache
    let value2 = cache
        .cache_or_compute(&key, 60, || async { Ok("should_not_compute".to_string()) })
        .await
        .unwrap();

    assert_eq!(value2, "computed_value");

    cleanup_redis_keys(&cache, &prefix).await;
}
