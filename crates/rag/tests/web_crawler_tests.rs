//! Web crawler unit and integration tests

mod common;

use anyhow::Result;
use ai_agent_common::CollectionTier;
use ai_agent_rag::web_crawler::WebCrawlerRetriever;
use ai_agent_rag::retriever::RetrieverSource;
use common::{
    mock_web_server::create_test_server,
    init_test_logging, create_test_config, create_test_embedding_client,
    create_test_qdrant_client, create_test_redis_client, test_collection_name,
    cleanup_test_collections, create_test_project_scope
};
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// Basic Functionality Tests
// ============================================================================

#[tokio::test]
async fn test_web_crawler_initialization() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant,
        redis,
        embedder,
        config,
    ).await;
    
    assert!(crawler.is_ok(), "Should initialize web crawler successfully");
    
    Ok(())
}

#[tokio::test]
async fn test_web_crawler_disabled() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    config.rag.web_crawler.enabled = false;
    
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    // Should still initialize but won't be used in MultiSourceRetriever
    let crawler = WebCrawlerRetriever::new(
        qdrant,
        redis,
        embedder,
        config,
    ).await;
    
    assert!(crawler.is_ok(), "Should initialize even when disabled");
    
    Ok(())
}

// ============================================================================
// Mock Web Crawling Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires test infrastructure
async fn test_crawl_rust_documentation() -> Result<()> {
    init_test_logging();
    
    let mock_server = create_test_server().await?;
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config,
    ).await?;
    
    let rust_doc_url = mock_server.url("/rust/async");
    let queries = vec![(CollectionTier::Online, rust_doc_url)];
    let project_scope = create_test_project_scope();
    
    let fragments = crawler.retrieve(queries, project_scope).await?;
    
    assert!(!fragments.is_empty(), "Should retrieve fragments from mock server");
    
    // Check that fragments contain expected content
    let content_found = fragments.iter().any(|f| 
        f.content.contains("async") || f.content.contains("Rust")
    );
    assert!(content_found, "Should contain relevant async/Rust content");
    
    // Cleanup test collections
    let collections = vec![
        config.rag.web_crawler.web_content_collection,
        config.rag.web_crawler.web_query_cache_collection,
    ];
    cleanup_test_collections(&qdrant, &collections).await;
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires test infrastructure
async fn test_semantic_caching() -> Result<()> {
    init_test_logging();
    
    let mock_server = create_test_server().await?;
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    let rust_doc_url = mock_server.url("/rust/async");
    let project_scope = create_test_project_scope();
    
    // First query - should crawl and cache
    let queries1 = vec![(CollectionTier::Online, rust_doc_url.clone())];
    let start_time = std::time::Instant::now();
    let fragments1 = crawler.retrieve(queries1, project_scope.clone()).await?;
    let first_duration = start_time.elapsed();
    
    assert!(!fragments1.is_empty(), "First query should return fragments");
    
    // Small delay to ensure cache is written
    sleep(Duration::from_millis(100)).await;
    
    // Similar query - should hit semantic cache
    let queries2 = vec![(CollectionTier::Online, rust_doc_url)];
    let start_time = std::time::Instant::now();
    let fragments2 = crawler.retrieve(queries2, project_scope).await?;
    let second_duration = start_time.elapsed();
    
    assert!(!fragments2.is_empty(), "Cached query should return fragments");
    
    // Note: In real scenarios, cached queries would be faster
    // But in tests with mock servers, the difference might not be significant
    println!("First query: {:?}, Second query: {:?}", first_duration, second_duration);
    
    // Cleanup
    let collections = vec![
        config.rag.web_crawler.web_content_collection,
        config.rag.web_crawler.web_query_cache_collection,
    ];
    cleanup_test_collections(&qdrant, &collections).await;
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires test infrastructure
async fn test_content_deduplication() -> Result<()> {
    init_test_logging();
    
    let mock_server = create_test_server().await?;
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    let rust_doc_url = mock_server.url("/rust/async");
    let project_scope = create_test_project_scope();
    
    // Query the same URL multiple times in one request
    let queries = vec![
        (CollectionTier::Online, rust_doc_url.clone()),
        (CollectionTier::Online, rust_doc_url.clone()),
        (CollectionTier::Online, rust_doc_url),
    ];
    
    let fragments = crawler.retrieve(queries, project_scope).await?;
    
    assert!(!fragments.is_empty(), "Should return fragments");
    
    // Check for deduplication - should not have exact duplicate content
    let mut content_hashes = std::collections::HashSet::new();
    let mut duplicates = 0;
    
    for fragment in &fragments {
        let hash = sha2::Digest::digest(fragment.content.as_bytes());
        if !content_hashes.insert(hash) {
            duplicates += 1;
        }
    }
    
    // Should have minimal duplicates due to deduplication
    assert!(duplicates < fragments.len() / 2, 
        "Should have effective deduplication. Duplicates: {}, Total: {}", 
        duplicates, fragments.len());
    
    // Cleanup
    let collections = vec![
        config.rag.web_crawler.web_content_collection,
        config.rag.web_crawler.web_query_cache_collection,
    ];
    cleanup_test_collections(&qdrant, &collections).await;
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires test infrastructure
async fn test_content_chunking_and_metadata() -> Result<()> {
    init_test_logging();
    
    let mock_server = create_test_server().await?;
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    let concepts_url = mock_server.url("/concepts/error-handling");
    let queries = vec![(CollectionTier::Online, concepts_url.clone())];
    let project_scope = create_test_project_scope();
    
    let fragments = crawler.retrieve(queries, project_scope).await?;
    
    assert!(!fragments.is_empty(), "Should retrieve and chunk content");
    
    // Check chunking - should have multiple fragments for large content
    assert!(fragments.len() > 1, "Large content should be chunked into multiple fragments");
    
    // Check metadata
    for fragment in &fragments {
        // Should have web content location
        match &fragment.metadata.location {
            ai_agent_common::Location::WebContent { url, title, .. } => {
                assert!(url.contains("error-handling"), "URL should be preserved");
                // Title might be extracted or None
            }
            _ => panic!("Fragment should have WebContent location"),
        }
        
        // Should have annotations with tags
        if let Some(annotations) = &fragment.metadata.annotations {
            if let Some(tags) = &annotations.tags {
                let has_web_tag = tags.iter().any(|tag| 
                    matches!(tag, ai_agent_common::TagContextFragment::TAG(t) if t == "web_content")
                );
                assert!(has_web_tag, "Should have web_content tag");
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
// Error Handling Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires test infrastructure
async fn test_handle_404_error() -> Result<()> {
    init_test_logging();
    
    let mock_server = create_test_server().await?;
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    let not_found_url = mock_server.url("/not-found");
    let queries = vec![(CollectionTier::Online, not_found_url)];
    let project_scope = create_test_project_scope();
    
    // Should handle 404 gracefully and return empty results
    let fragments = crawler.retrieve(queries, project_scope).await?;
    
    // Should not panic and return empty or minimal results
    assert!(fragments.len() == 0, "404 should result in no fragments");
    
    // Cleanup
    let collections = vec![
        config.rag.web_crawler.web_content_collection,
        config.rag.web_crawler.web_query_cache_collection,
    ];
    cleanup_test_collections(&qdrant, &collections).await;
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires test infrastructure
async fn test_handle_timeout() -> Result<()> {
    init_test_logging();
    
    let mock_server = create_test_server().await?;
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    let slow_url = mock_server.url("/slow");
    let queries = vec![(CollectionTier::Online, slow_url)];
    let project_scope = create_test_project_scope();
    
    // Should handle timeout gracefully
    let fragments = crawler.retrieve(queries, project_scope).await?;
    
    // Should not panic and return empty results for timeout
    assert!(fragments.len() == 0, "Timeout should result in no fragments");
    
    // Cleanup
    let collections = vec![
        config.rag.web_crawler.web_content_collection,
        config.rag.web_crawler.web_query_cache_collection,
    ];
    cleanup_test_collections(&qdrant, &collections).await;
    
    Ok(())
}

// ============================================================================
// Multiple Source Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires test infrastructure
async fn test_multiple_urls() -> Result<()> {
    init_test_logging();
    
    let mock_server = create_test_server().await?;
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    let queries = vec![
        (CollectionTier::Online, mock_server.url("/rust/async")),
        (CollectionTier::Online, mock_server.url("/python/asyncio")),
        (CollectionTier::Online, mock_server.url("/concepts/error-handling")),
    ];
    let project_scope = create_test_project_scope();
    
    let fragments = crawler.retrieve(queries, project_scope).await?;
    
    assert!(!fragments.is_empty(), "Should retrieve from multiple URLs");
    
    // Should have content from different sources
    let rust_content = fragments.iter().any(|f| 
        f.content.to_lowercase().contains("rust")
    );
    let python_content = fragments.iter().any(|f| 
        f.content.to_lowercase().contains("python")
    );
    let error_content = fragments.iter().any(|f| 
        f.content.to_lowercase().contains("error")
    );
    
    assert!(rust_content || python_content || error_content, 
        "Should have content from different sources");
    
    // Cleanup
    let collections = vec![
        config.rag.web_crawler.web_content_collection,
        config.rag.web_crawler.web_query_cache_collection,
    ];
    cleanup_test_collections(&qdrant, &collections).await;
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires test infrastructure
async fn test_non_online_tier_filtering() -> Result<()> {
    init_test_logging();
    
    let mock_server = create_test_server().await?;
    let config = create_test_config();
    let qdrant = create_test_qdrant_client()?;
    let redis = create_test_redis_client().await?;
    let embedder = create_test_embedding_client()?;
    
    let crawler = WebCrawlerRetriever::new(
        qdrant.clone(),
        redis,
        embedder,
        config.clone(),
    ).await?;
    
    // Mix of online and non-online queries
    let queries = vec![
        (CollectionTier::Workspace, "local code query".to_string()),
        (CollectionTier::Personal, "personal docs query".to_string()),
        (CollectionTier::Online, mock_server.url("/rust/async")),
    ];
    let project_scope = create_test_project_scope();
    
    let fragments = crawler.retrieve(queries, project_scope).await?;
    
    // Should only process Online tier queries
    // Should have some fragments from the online query
    if !fragments.is_empty() {
        // All fragments should be from web content
        for fragment in &fragments {
            match &fragment.metadata.location {
                ai_agent_common::Location::WebContent { .. } => {
                    // Good - this is web content
                }
                _ => panic!("Web crawler should only return web content"),
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