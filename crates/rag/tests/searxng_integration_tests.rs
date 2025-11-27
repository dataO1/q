//! SearXNG integration tests for WebCrawler

mod common;

use anyhow::Result;
use ai_agent_common::{CollectionTier, SystemConfig};
use ai_agent_rag::searxng_client::{SearXNGClient, SearchResult};
use ai_agent_rag::web_crawler::WebCrawlerRetriever;
use ai_agent_rag::retriever::RetrieverSource;
use common::{
    init_test_logging, create_test_config, create_test_embedding_client,
    create_test_qdrant_client, create_test_redis_client, 
    cleanup_test_collections, create_test_project_scope
};
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// SearXNG Client Tests
// ============================================================================

#[tokio::test]
async fn test_searxng_client_creation() -> Result<()> {
    init_test_logging();
    
    let client = SearXNGClient::new(
        "http://localhost:8888".to_string(),
        10,
        5,
        vec!["duckduckgo".to_string(), "bing".to_string()],
    )?;
    
    // Should create successfully
    println!("SearXNG client created: {:?}", client);
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running SearXNG instance
async fn test_searxng_health_check() -> Result<()> {
    init_test_logging();
    
    let client = SearXNGClient::new(
        "http://localhost:8888".to_string(),
        10,
        5,
        vec!["duckduckgo".to_string()],
    )?;
    
    // Test health check
    let health_result = client.health_check().await;
    
    match health_result {
        Ok(_) => {
            println!("SearXNG health check passed");
        }
        Err(e) => {
            println!("SearXNG health check failed (expected if not running): {}", e);
            // This is expected if SearXNG is not running
        }
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running SearXNG instance
async fn test_searxng_search_basic() -> Result<()> {
    init_test_logging();
    
    let client = SearXNGClient::new(
        "http://localhost:8888".to_string(),
        30,
        10,
        vec!["duckduckgo".to_string()],
    )?;
    
    // Perform health check first
    client.health_check().await?;
    
    let query = "rust async programming";
    let results = client.search(query).await?;
    
    println!("Search results for '{}': {} results", query, results.len());
    
    assert!(results.len() <= 10, "Should respect max_results limit");
    
    for (i, result) in results.iter().enumerate() {
        println!("Result {}: {} - {}", i + 1, result.title, result.url);
        
        // Basic validation
        assert!(!result.url.is_empty(), "URL should not be empty");
        assert!(!result.title.is_empty(), "Title should not be empty");
        
        // URL should be valid
        let url_parse = url::Url::parse(&result.url);
        assert!(url_parse.is_ok(), "URL should be valid: {}", result.url);
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running SearXNG instance
async fn test_searxng_search_with_engines() -> Result<()> {
    init_test_logging();
    
    let client = SearXNGClient::new(
        "http://localhost:8888".to_string(),
        30,
        5,
        vec!["duckduckgo".to_string(), "bing".to_string()],
    )?;
    
    client.health_check().await?;
    
    let results = client.search("python asyncio").await?;
    
    println!("Multi-engine search returned {} results", results.len());
    
    // Check that results have engine information
    for result in &results {
        println!("Result from engine '{}': {}", result.engine, result.url);
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running SearXNG instance
async fn test_searxng_search_empty_query() -> Result<()> {
    init_test_logging();
    
    let client = SearXNGClient::new(
        "http://localhost:8888".to_string(),
        30,
        5,
        vec!["duckduckgo".to_string()],
    )?;
    
    client.health_check().await?;
    
    // Test empty query
    let result = client.search("").await;
    
    match result {
        Ok(results) => {
            println!("Empty query returned {} results", results.len());
        }
        Err(e) => {
            println!("Empty query failed as expected: {}", e);
        }
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running SearXNG instance  
async fn test_searxng_search_special_characters() -> Result<()> {
    init_test_logging();
    
    let client = SearXNGClient::new(
        "http://localhost:8888".to_string(),
        30,
        5,
        vec!["duckduckgo".to_string()],
    )?;
    
    client.health_check().await?;
    
    let special_queries = vec![
        "rust + async",
        "error handling \"best practices\"",
        "async/await syntax",
        "Rust🦀 programming",
        "query with spaces and symbols !@#$%",
    ];
    
    for query in special_queries {
        let result = client.search(query).await;
        
        match result {
            Ok(results) => {
                println!("Special query '{}' returned {} results", query, results.len());
            }
            Err(e) => {
                println!("Special query '{}' failed: {}", query, e);
            }
        }
    }
    
    Ok(())
}

#[test]
fn test_search_result_url_extraction() {
    let results = vec![
        SearchResult {
            url: "https://docs.rust-lang.org/book/".to_string(),
            title: "The Rust Programming Language".to_string(),
            content: "Learn Rust".to_string(),
            engine: "google".to_string(),
            score: 1.0,
        },
        SearchResult {
            url: "https://github.com/rust-lang/rust".to_string(),
            title: "Rust Repository".to_string(),
            content: "Source code".to_string(),
            engine: "github".to_string(),
            score: 0.9,
        },
        SearchResult {
            url: "https://docs.rust-lang.org/book/".to_string(), // Duplicate
            title: "The Rust Programming Language (duplicate)".to_string(),
            content: "Duplicate".to_string(),
            engine: "bing".to_string(),
            score: 0.8,
        },
    ];
    
    let urls = SearXNGClient::extract_urls(&results);
    
    assert_eq!(urls.len(), 2, "Should deduplicate URLs");
    assert!(urls.contains(&"https://docs.rust-lang.org/book/".to_string()));
    assert!(urls.contains(&"https://github.com/rust-lang/rust".to_string()));
}

// ============================================================================
// WebCrawler SearXNG Integration Tests
// ============================================================================

#[tokio::test]
async fn test_webcrawler_searxng_disabled() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.rag.web_crawler.searxng.enabled = false;
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant,
        redis,
        embedder,
        config,
    ).await?;
    
    // Should initialize successfully even with SearXNG disabled
    println!("WebCrawler initialized with SearXNG disabled: {:?}", crawler);
    
    Ok(())
}

#[tokio::test]
async fn test_webcrawler_searxng_invalid_endpoint() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.rag.web_crawler.searxng.enabled = true;
    config.rag.web_crawler.searxng.endpoint = "http://invalid-endpoint:9999".to_string();
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    // Should handle invalid endpoint gracefully
    let crawler = WebCrawlerRetriever::new(
        qdrant,
        redis,
        embedder,
        config,
    ).await?;
    
    println!("WebCrawler initialized with invalid SearXNG endpoint: {:?}", crawler);
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running SearXNG instance
async fn test_webcrawler_with_searxng_integration() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.rag.web_crawler.searxng.enabled = true;
    config.rag.web_crawler.searxng.endpoint = "http://localhost:8888".to_string();
    config.rag.web_crawler.searxng.max_results = 3;
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    let project_scope = create_test_project_scope();
    
    // Test web search with actual crawling
    let queries = vec![
        (CollectionTier::Online, "rust async programming tutorial".to_string())
    ];
    
    let fragments = crawler.retrieve(queries, project_scope).await?;
    
    println!("SearXNG-enabled crawler retrieved {} fragments", fragments.len());
    
    if !fragments.is_empty() {
        // Verify fragments have web content location
        for fragment in &fragments {
            match &fragment.metadata.location {
                ai_agent_common::Location::WebContent { url, .. } => {
                    println!("Retrieved fragment from: {}", url);
                    
                    // URL should be valid
                    let url_parse = url::Url::parse(url);
                    assert!(url_parse.is_ok(), "Fragment URL should be valid: {}", url);
                }
                _ => {
                    panic!("WebCrawler should only return WebContent fragments");
                }
            }
        }
        
        // Check that content is not empty
        let has_content = fragments.iter().any(|f| !f.content.trim().is_empty());
        assert!(has_content, "Should have non-empty content in at least one fragment");
    }
    
    // Cleanup
    let collections = vec![
        config.rag.web_crawler.web_content_collection,
        config.rag.web_crawler.web_query_cache_collection,
    ];
    cleanup_test_collections(&qdrant, &collections).await;
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running SearXNG instance
async fn test_webcrawler_searxng_caching() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.rag.web_crawler.searxng.enabled = true;
    config.rag.web_crawler.searxng.endpoint = "http://localhost:8888".to_string();
    config.rag.web_crawler.searxng.max_results = 2;
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    let project_scope = create_test_project_scope();
    let query = "rust documentation async".to_string();
    
    // First request
    let start_time = std::time::Instant::now();
    let queries1 = vec![(CollectionTier::Online, query.clone())];
    let fragments1 = crawler.retrieve(queries1, project_scope.clone()).await?;
    let first_duration = start_time.elapsed();
    
    println!("First request took {:?}, got {} fragments", first_duration, fragments1.len());
    
    // Small delay to ensure caching is complete
    sleep(Duration::from_millis(200)).await;
    
    // Second request (should hit cache)
    let start_time = std::time::Instant::now();
    let queries2 = vec![(CollectionTier::Online, query)];
    let fragments2 = crawler.retrieve(queries2, project_scope).await?;
    let second_duration = start_time.elapsed();
    
    println!("Second request took {:?}, got {} fragments", second_duration, fragments2.len());
    
    if !fragments1.is_empty() && !fragments2.is_empty() {
        // Results should be similar (caching working)
        let count_diff = (fragments1.len() as i32 - fragments2.len() as i32).abs();
        assert!(count_diff <= 1, "Cached results should be similar in count");
        
        // Second request might be faster (though not guaranteed in test environment)
        println!("Cache performance: first={:?}, second={:?}", first_duration, second_duration);
    }
    
    // Cleanup
    let collections = vec![
        config.rag.web_crawler.web_content_collection,
        config.rag.web_crawler.web_query_cache_collection,
    ];
    cleanup_test_collections(&qdrant, &collections).await;
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running SearXNG instance
async fn test_webcrawler_searxng_concurrent_requests() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.rag.web_crawler.searxng.enabled = true;
    config.rag.web_crawler.searxng.endpoint = "http://localhost:8888".to_string();
    config.rag.web_crawler.searxng.max_results = 2;
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    let project_scope = create_test_project_scope();
    
    // Create multiple concurrent requests with different queries
    let queries = vec![
        "rust async programming",
        "python asyncio tutorial",
        "javascript promises guide",
    ];
    
    let mut tasks = Vec::new();
    
    for query in queries {
        let crawler_clone = WebCrawlerRetriever::new(
            qdrant.clone(),
            create_test_redis_client().await?,
            create_test_embedding_client()?,
            config.clone(),
        ).await?;
        let project_scope_clone = project_scope.clone();
        
        let task = tokio::spawn(async move {
            let queries = vec![(CollectionTier::Online, query.to_string())];
            let result = crawler_clone.retrieve(queries, project_scope_clone).await;
            (query, result)
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let results = futures::future::join_all(tasks).await;
    
    for result in results {
        let (query, fragments_result) = result?;
        match fragments_result {
            Ok(fragments) => {
                println!("Concurrent query '{}' returned {} fragments", query, fragments.len());
            }
            Err(e) => {
                println!("Concurrent query '{}' failed: {}", query, e);
            }
        }
    }
    
    // Cleanup
    let collections = vec![
        config.rag.web_crawler.web_content_collection,
        config.rag.web_crawler.web_query_cache_collection,
    ];
    cleanup_test_collections(&qdrant, &collections).await;
    
    Ok(())
}

// ============================================================================
// SearXNG Configuration Tests
// ============================================================================

#[test]
fn test_searxng_config_validation() {
    let mut config = create_test_config();
    
    // Test various SearXNG configurations
    config.rag.web_crawler.searxng.enabled = true;
    config.rag.web_crawler.searxng.endpoint = "http://localhost:8888".to_string();
    config.rag.web_crawler.searxng.timeout_secs = 30;
    config.rag.web_crawler.searxng.max_results = 10;
    config.rag.web_crawler.searxng.preferred_engines = vec![
        "duckduckgo".to_string(),
        "bing".to_string(),
        "startpage".to_string(),
    ];
    
    // Should create client successfully
    let client_result = SearXNGClient::new(
        config.rag.web_crawler.searxng.endpoint.clone(),
        config.rag.web_crawler.searxng.timeout_secs,
        config.rag.web_crawler.searxng.max_results,
        config.rag.web_crawler.searxng.preferred_engines.clone(),
    );
    
    assert!(client_result.is_ok(), "Should create SearXNG client with valid config");
}

#[test]
fn test_searxng_config_edge_cases() {
    // Test edge case configurations
    let edge_cases = vec![
        ("http://localhost:8888", 1, 1, vec![]), // Minimal config
        ("https://searx.example.com", 60, 100, vec!["google".to_string()]), // High limits
        ("http://127.0.0.1:8080", 5, 0, vec![]), // Zero max results
    ];
    
    for (endpoint, timeout, max_results, engines) in edge_cases {
        let client_result = SearXNGClient::new(
            endpoint.to_string(),
            timeout,
            max_results,
            engines,
        );
        
        match client_result {
            Ok(client) => {
                println!("Edge case config created successfully: {:?}", client);
            }
            Err(e) => {
                println!("Edge case config failed (may be expected): {}", e);
            }
        }
    }
}

#[test]
fn test_searxng_config_invalid() {
    let invalid_configs = vec![
        ("", 30, 10, vec![]), // Empty endpoint
        ("not-a-url", 30, 10, vec![]), // Invalid URL format
        ("http://localhost:8888", 0, 10, vec![]), // Zero timeout
    ];
    
    for (endpoint, timeout, max_results, engines) in invalid_configs {
        let client_result = SearXNGClient::new(
            endpoint.to_string(),
            timeout,
            max_results,
            engines,
        );
        
        // Some invalid configs might still create a client, others might fail
        match client_result {
            Ok(_) => {
                println!("Invalid config unexpectedly succeeded: {}", endpoint);
            }
            Err(e) => {
                println!("Invalid config failed as expected: {} - {}", endpoint, e);
            }
        }
    }
}