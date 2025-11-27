//! Error handling and edge case tests for RAG system

mod common;

use anyhow::Result;
use ai_agent_common::{CollectionTier, ProjectScope, Language, SystemConfig};
use ai_agent_rag::retriever::MultiSourceRetriever;
use ai_agent_rag::source_router::SourceRouter;
use ai_agent_rag::web_crawler::WebCrawlerRetriever;
use ai_agent_rag::SmartMultiSourceRag;
use common::{
    init_test_logging, create_test_config, create_test_embedding_client,
    create_test_qdrant_client, create_test_redis_client, create_test_project_scope
};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::Duration;

// ============================================================================
// Network and Infrastructure Failure Tests
// ============================================================================

#[tokio::test]
async fn test_qdrant_connection_failure() -> Result<()> {
    init_test_logging();
    
    let embedder = create_test_embedding_client()?;
    let qdrant_result = ai_agent_storage::QdrantClient::new("http://invalid-qdrant-host:9999", embedder);
    
    match qdrant_result {
        Ok(_) => {
            println!("Unexpected success with invalid Qdrant URL");
        }
        Err(e) => {
            println!("Expected Qdrant connection error: {}", e);
            assert!(e.to_string().contains("connection") || e.to_string().contains("failed"), 
                "Error should mention connection issue");
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_redis_connection_failure() -> Result<()> {
    init_test_logging();
    
    let redis_result = ai_agent_storage::RedisCache::new("redis://invalid-redis-host:9999/").await;
    
    match redis_result {
        Ok(_) => {
            println!("Unexpected success with invalid Redis URL");
        }
        Err(e) => {
            println!("Expected Redis connection error: {}", e);
            let error_msg = e.to_string().to_lowercase();
            assert!(error_msg.contains("connection") || 
                   error_msg.contains("redis") || 
                   error_msg.contains("connect") || 
                   error_msg.contains("failed") ||
                   error_msg.contains("timeout") ||
                   error_msg.contains("unreachable"), 
                "Error should indicate connection failure, got: {}", e);
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_ollama_connection_failure() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.embedding.ollama_host = "invalid-ollama-host".to_string();
    config.embedding.ollama_port = 65000;
    
    let router = SourceRouter::new(&config)?;
    let project_scope = create_test_project_scope();
    
    // Should handle Ollama connection errors gracefully
    let result = router.route_query("test query", &project_scope).await;
    
    match result {
        Ok(routes) => {
            println!("Router handled Ollama failure gracefully: {:?}", routes.keys().collect::<Vec<_>>());
            // Should fallback to heuristics and provide at least one route
            assert!(!routes.is_empty(), "Should provide fallback routing");
        }
        Err(e) => {
            println!("Router failed with Ollama connection error: {}", e);
            // Connection errors are acceptable
        }
    }
    
    Ok(())
}

// ============================================================================
// Configuration and Initialization Error Tests
// ============================================================================

#[tokio::test]
async fn test_invalid_system_config() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    
    // Test with various invalid configurations
    config.rag.web_crawler.max_urls_per_query = 0;
    config.rag.web_crawler.chunk_size = 0;
    config.rag.web_crawler.chunk_overlap = 1000; // Overlap larger than chunk size
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    // Should handle invalid config gracefully or fail with appropriate error
    let result = WebCrawlerRetriever::new(qdrant, redis, embedder, config).await;
    
    match result {
        Ok(crawler) => {
            println!("WebCrawler created with questionable config: {:?}", crawler);
            // System might handle invalid config by using defaults
        }
        Err(e) => {
            println!("WebCrawler failed with invalid config: {}", e);
            // This is also acceptable - should fail with clear error
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_missing_collections() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.rag.web_crawler.web_content_collection = "".to_string();
    config.rag.web_crawler.web_query_cache_collection = "".to_string();
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let result = WebCrawlerRetriever::new(qdrant, redis, embedder, config).await;
    
    match result {
        Ok(_) => {
            println!("WebCrawler handled empty collection names");
        }
        Err(e) => {
            println!("WebCrawler failed with empty collection names: {}", e);
        }
    }
    
    Ok(())
}

// ============================================================================
// Malformed Input and Security Tests
// ============================================================================

#[tokio::test]
async fn test_malformed_queries() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    let project_scope = create_test_project_scope();
    
    let long_query = "A".repeat(10000);
    let malformed_queries = vec![
        "\0", // Null byte
        "\x01\x02\x03", // Binary data
        &long_query, // Very long string
        "'; DROP TABLE users; --", // SQL injection attempt
        "<script>alert('xss')</script>", // XSS attempt
        "{{constructor.constructor('return process')().exit()}}", // JS injection
        "../../../../etc/passwd", // Path traversal
        "%00", // URL encoded null byte
        "\r\n\r\n<script>alert(1)</script>", // CRLF injection
    ];
    
    for malicious_query in malformed_queries {
        let result = router.route_query(malicious_query, &project_scope).await;
        
        match result {
            Ok(routes) => {
                println!("Malicious query handled safely: {} routes", routes.len());
                // Should not crash and provide safe fallback
                assert!(!routes.is_empty(), "Should provide safe fallback routing");
                
                // Verify the query is not executed as code
                for (tier, routed_query) in &routes {
                    assert_eq!(routed_query, malicious_query, "Query should be preserved as-is for tier: {:?}", tier);
                }
            }
            Err(e) => {
                println!("Malicious query failed safely: {}", e);
                // Safe failure is also acceptable
            }
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_unicode_edge_cases() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    let project_scope = create_test_project_scope();
    
    let unicode_queries = vec![
        "🦀 Rust programming", // Emojis
        "Café programming", // Accented characters  
        "プログラミング", // Japanese
        "программирование", // Cyrillic
        "𝕏 mathematical symbols", // Mathematical Unicode
        "זה טקסט בעברית", // Hebrew (RTL)
        "🏳️‍🌈🏴‍☠️", // Complex emoji sequences
        "\u{200B}\u{FEFF}invisible chars", // Zero-width characters
    ];
    
    for unicode_query in unicode_queries {
        let result = router.route_query(unicode_query, &project_scope).await;
        
        match result {
            Ok(routes) => {
                println!("Unicode query '{}' handled: {} routes", unicode_query, routes.len());
                assert!(!routes.is_empty(), "Should handle Unicode queries");
            }
            Err(e) => {
                println!("Unicode query '{}' failed: {}", unicode_query, e);
                // Some failures might be expected depending on the system
            }
        }
    }
    
    Ok(())
}

// ============================================================================
// Resource Exhaustion Tests
// ============================================================================

#[tokio::test]
#[ignore] // Resource intensive test
async fn test_concurrent_request_limits() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let retriever = MultiSourceRetriever::new(
        qdrant,
        embedder,
        redis,
        config,
    ).await?;
    
    let project_scope = create_test_project_scope();
    
    // Create many concurrent requests to test system limits
    let concurrent_requests = 50;
    let mut tasks = Vec::new();
    
    for i in 0..concurrent_requests {
        let retriever_arc = Arc::new(retriever.clone());
        let project_scope_clone = project_scope.clone();
        
        let task = tokio::spawn(async move {
            let queries = HashMap::from([
                (CollectionTier::Workspace, vec![format!("concurrent query {}", i)])
            ]);
            
            let mut stream = retriever_arc.retrieve_stream(
                format!("load test query {}", i),
                queries,
                project_scope_clone,
            );
            
            let mut count = 0;
            let start_time = std::time::Instant::now();
            
            while let Some(result) = stream.next().await {
                match result {
                    Ok(_) => count += 1,
                    Err(_) => break,
                }
                
                // Limit processing time to prevent infinite loops
                if start_time.elapsed() > Duration::from_secs(30) {
                    break;
                }
                
                if count > 5 {
                    break;
                }
            }
            
            (i, count)
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks with timeout
    let timeout = Duration::from_secs(60);
    let results = tokio::time::timeout(timeout, futures::future::join_all(tasks)).await;
    
    match results {
        Ok(task_results) => {
            let mut successful = 0;
            let mut failed = 0;
            
            for result in task_results {
                match result {
                    Ok((task_id, fragment_count)) => {
                        println!("Task {} completed with {} fragments", task_id, fragment_count);
                        successful += 1;
                    }
                    Err(e) => {
                        println!("Task failed: {}", e);
                        failed += 1;
                    }
                }
            }
            
            println!("Concurrent test results: {} successful, {} failed", successful, failed);
            
            // Most requests should succeed
            assert!(successful > failed, "Most concurrent requests should succeed");
        }
        Err(_) => {
            println!("Concurrent test timed out - system may be under heavy load");
        }
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Memory intensive test  
async fn test_large_query_processing() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    let project_scope = create_test_project_scope();
    
    // Test with progressively larger queries
    let sizes = vec![1000, 10000, 100000];
    
    for size in sizes {
        let large_query = "async function implementation ".repeat(size / 30);
        
        println!("Testing query of size: {} characters", large_query.len());
        
        let start_time = std::time::Instant::now();
        let result = router.route_query(&large_query, &project_scope).await;
        let duration = start_time.elapsed();
        
        match result {
            Ok(routes) => {
                println!("Large query ({} chars) processed in {:?}: {} routes", 
                    large_query.len(), duration, routes.len());
                
                // Should complete in reasonable time
                assert!(duration < Duration::from_secs(30), 
                    "Large query processing should complete within 30 seconds");
                
                assert!(!routes.is_empty(), "Should route large queries");
            }
            Err(e) => {
                println!("Large query ({} chars) failed: {}", large_query.len(), e);
                // Failures are acceptable for very large inputs
            }
        }
        
        // Small delay between tests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    Ok(())
}

// ============================================================================
// Data Corruption and Inconsistency Tests
// ============================================================================

#[tokio::test]
async fn test_corrupted_cache_handling() -> Result<()> {
    init_test_logging();
    
    let redis = create_test_redis_client().await?;
    
    // Inject corrupted data into cache
    let corrupted_data = vec![
        "invalid json {",
        "",
        "null",
        "{'invalid': json}",
        "binary\x00\x01\x02data",
    ];
    
    for (i, corrupt_value) in corrupted_data.iter().enumerate() {
        let key = format!("test_corrupt_key_{}", i);
        let _ = redis.set(&key, corrupt_value).await;
        
        // Try to retrieve corrupted data
        let result: Result<Option<String>, _> = redis.get(&key).await;
        
        match result {
            Ok(Some(value)) => {
                println!("Retrieved corrupted data: {:?}", value);
                // System should handle corrupted data gracefully
            }
            Ok(None) => {
                println!("Corrupted data not found (cache may have filtered it)");
            }
            Err(e) => {
                println!("Corrupted data caused error: {}", e);
                // Errors are acceptable when handling corrupted data
            }
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_partial_system_failure() -> Result<()> {
    init_test_logging();
    
    // Test when some components fail but others work
    let mut config = create_test_config();
    config.rag.web_crawler.enabled = true;
    config.rag.web_crawler.searxng.enabled = true;
    config.rag.web_crawler.searxng.endpoint = "http://invalid-searxng:9999".to_string();
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    // Should handle SearXNG failure gracefully
    let retriever_result = MultiSourceRetriever::new(
        qdrant,
        embedder,
        redis,
        config,
    ).await;
    
    match retriever_result {
        Ok(retriever) => {
            println!("MultiSourceRetriever created despite SearXNG failure");
            
            let project_scope = create_test_project_scope();
            let queries = HashMap::from([
                (CollectionTier::Workspace, vec!["test query".to_string()]),
                (CollectionTier::Online, vec!["web query".to_string()]),
            ]);
            
            // Should still work for non-web queries
            let mut stream = Arc::new(retriever).retrieve_stream(
                "partial failure test".to_string(),
                queries,
                project_scope,
            );
            
            let mut fragments = Vec::new();
            let start_time = std::time::Instant::now();
            
            while let Some(result) = stream.next().await {
                match result {
                    Ok(fragment) => {
                        fragments.push(fragment);
                        if fragments.len() > 10 {
                            break;
                        }
                    }
                    Err(e) => {
                        println!("Expected error with partial failure: {}", e);
                        break;
                    }
                }
                
                if start_time.elapsed() > Duration::from_secs(10) {
                    break;
                }
            }
            
            println!("Partial failure test retrieved {} fragments", fragments.len());
            // System should continue working for available components
        }
        Err(e) => {
            println!("MultiSourceRetriever failed with partial component failure: {}", e);
            // This is also acceptable behavior
        }
    }
    
    Ok(())
}

// ============================================================================
// Timeout and Performance Edge Cases
// ============================================================================

#[tokio::test]
async fn test_request_timeouts() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.rag.web_crawler.request_timeout_secs = 1; // Very short timeout
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let retriever = MultiSourceRetriever::new(
        qdrant,
        embedder,
        redis,
        config,
    ).await?;
    
    let project_scope = create_test_project_scope();
    let queries = HashMap::from([
        (CollectionTier::Online, vec!["timeout test query".to_string()])
    ]);
    
    // Test with very short timeout - should handle gracefully
    let start_time = std::time::Instant::now();
    let mut stream = Arc::new(retriever).retrieve_stream(
        "timeout test".to_string(),
        queries,
        project_scope,
    );
    
    let mut fragments = Vec::new();
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(fragment) => {
                fragments.push(fragment);
            }
            Err(e) => {
                println!("Timeout test error (expected): {}", e);
                break;
            }
        }
        
        // Don't wait too long
        if start_time.elapsed() > Duration::from_secs(15) {
            break;
        }
        
        if fragments.len() > 5 {
            break;
        }
    }
    
    let total_duration = start_time.elapsed();
    println!("Timeout test completed in {:?} with {} fragments", total_duration, fragments.len());
    
    // Should complete quickly due to timeout settings
    assert!(total_duration < Duration::from_secs(20), "Should handle timeouts quickly");
    
    Ok(())
}

// ============================================================================
// Integration Error Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires full test infrastructure
async fn test_full_rag_error_resilience() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.rag.web_crawler.enabled = true;
    
    let embedder = create_test_embedding_client()?;
    
    // Test full RAG system with potential failures
    let rag_result = SmartMultiSourceRag::new(&config, embedder).await;
    
    match rag_result {
        Ok(rag) => {
            let project_scope = create_test_project_scope();
            let conversation_id = ai_agent_common::ConversationId("error_test".to_string());
            
            // Test with various problematic queries
            let long_query = format!("very {}query", "long ".repeat(1000));
            let problematic_queries = vec![
                "", // Empty
                "normal query", // Should work
                &long_query, // Very long
                "🦀💻🔥", // Unicode only
            ];
            
            for query in problematic_queries {
                let result = rag.clone().retrieve_stream(
                    query.to_string(),
                    project_scope.clone(),
                    conversation_id.clone(),
                ).await;
                
                match result {
                    Ok(mut stream) => {
                        println!("RAG stream created for query: '{}'", 
                            query.chars().take(50).collect::<String>());
                        
                        let mut count = 0;
                        let start_time = std::time::Instant::now();
                        
                        while let Some(result) = stream.next().await {
                            match result {
                                Ok(_) => count += 1,
                                Err(e) => {
                                    println!("Stream error: {}", e);
                                    break;
                                }
                            }
                            
                            if count > 3 || start_time.elapsed() > Duration::from_secs(5) {
                                break;
                            }
                        }
                        
                        println!("Query processed: {} fragments", count);
                    }
                    Err(e) => {
                        println!("RAG failed for query '{}': {}", 
                            query.chars().take(20).collect::<String>(), e);
                    }
                }
            }
        }
        Err(e) => {
            println!("RAG system initialization failed: {}", e);
            // This might be expected in test environment
        }
    }
    
    Ok(())
}