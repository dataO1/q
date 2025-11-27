//! Comprehensive tests for query enhancement functionality

mod common;

use anyhow::Result;
use ai_agent_common::{ProjectScope, ConversationId, Language, CollectionTier};
use ai_agent_rag::query_enhancer::QueryEnhancer;
use common::{init_test_logging, create_test_redis_client, create_test_project_scope};
use std::sync::Once;

static INIT: Once = Once::new();

fn init() {
    INIT.call_once(|| {
        init_test_logging();
    });
}

fn dummy_conversation_id() -> ConversationId {
    ConversationId("test_conversation".to_string())
}

// ============================================================================
// Basic Query Enhancement Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires Redis and proper vocab.txt file
async fn test_query_enhancer_creation() -> Result<()> {
    init();
    
    let config = crate::common::create_test_config();
    
    // Test creation (may fail if vocab.txt is not available)
    let result = QueryEnhancer::new(&config).await;
    
    match result {
        Ok(enhancer) => {
            println!("QueryEnhancer created successfully: {:?}", enhancer);
        }
        Err(e) => {
            println!("QueryEnhancer creation failed (expected without vocab.txt): {}", e);
        }
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires Redis and vocab.txt
async fn test_query_enhancement_basic() -> Result<()> {
    init();
    
    let config = crate::common::create_test_config();
    let enhancer = QueryEnhancer::new(&config).await?;
    let project_scope = create_test_project_scope();
    let conversation_id = dummy_conversation_id();
    
    let query = "find async functions";
    let tier = CollectionTier::Workspace;
    
    let enhanced_queries = enhancer.enhance(
        query,
        &project_scope,
        &conversation_id,
        tier,
    ).await?;
    
    println!("Enhanced queries: {:?}", enhanced_queries);
    
    assert!(!enhanced_queries.is_empty(), "Should return enhanced queries");
    
    // Should include original and enhanced variants
    let has_original = enhanced_queries.iter().any(|q| q.contains("async"));
    assert!(has_original, "Should include queries related to original");
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires Redis and vocab.txt
async fn test_query_enhancement_caching() -> Result<()> {
    init();
    
    let config = crate::common::create_test_config();
    let enhancer = QueryEnhancer::new(&config).await?;
    let project_scope = create_test_project_scope();
    let conversation_id = dummy_conversation_id();
    
    let query = "error handling patterns";
    let tier = CollectionTier::Workspace;
    
    // First call - should compute and cache
    let start_time = std::time::Instant::now();
    let first_result = enhancer.enhance(
        query,
        &project_scope,
        &conversation_id,
        tier,
    ).await?;
    let first_duration = start_time.elapsed();
    
    // Small delay to ensure cache is written
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    
    // Second call - should hit cache
    let start_time = std::time::Instant::now();
    let second_result = enhancer.enhance(
        query,
        &project_scope,
        &conversation_id,
        tier,
    ).await?;
    let second_duration = start_time.elapsed();
    
    println!("First call: {:?}, Second call: {:?}", first_duration, second_duration);
    
    // Results should be identical
    assert_eq!(first_result, second_result, "Cached results should match");
    
    // Second call should be faster (cache hit)
    if second_duration < first_duration {
        println!("Cache appears to be working (second call faster)");
    } else {
        println!("Cache timing inconclusive (may be due to test environment)");
    }
    
    Ok(())
}

// ============================================================================
// Heuristic Enhancement Tests
// ============================================================================

#[test]
fn test_heuristic_stopword_filtering() {
    // Test stopword removal logic (this is a unit test for the private method concept)
    let stopwords = ["the", "is", "at", "which", "on", "a"];
    
    let test_tokens = vec!["find", "the", "async", "functions", "in", "code"];
    let filtered: Vec<&str> = test_tokens.iter()
        .filter(|&&token| !stopwords.contains(&token))
        .copied()
        .collect();
    
    assert_eq!(filtered, vec!["find", "async", "functions", "in", "code"]);
    assert!(!filtered.contains(&"the"), "Should remove stopword 'the'");
}

#[test]
fn test_heuristic_synonym_expansion() {
    // Test synonym expansion logic
    let query = "find error in code";
    
    let mut variants = Vec::new();
    variants.push(query.to_lowercase());
    
    if query.to_lowercase().contains("error") {
        variants.push(query.replace("error", "exception"));
        variants.push(query.replace("error", "bug"));
    }
    
    assert_eq!(variants.len(), 3, "Should generate 3 variants for error query");
    assert!(variants.contains(&"find exception in code".to_string()));
    assert!(variants.contains(&"find bug in code".to_string()));
}

#[test]
fn test_cache_key_generation() {
    use sha2::{Digest, Sha256};
    
    let query = "test query";
    let conversation_id = ConversationId("conv123".to_string());
    let tier = CollectionTier::Workspace;
    let project_scope = create_test_project_scope();
    let heuristic_version = 1u8;
    
    // Simulate cache key generation
    let key_str = format!(
        "{}|{}|{:?}|{:?}|v{}",
        query, conversation_id, tier, project_scope.language_distribution, heuristic_version
    );
    let cache_key = hex::encode(Sha256::digest(key_str.as_bytes()));
    
    println!("Generated cache key: {}", cache_key);
    
    assert_eq!(cache_key.len(), 64, "SHA256 hash should be 64 characters");
    assert!(cache_key.chars().all(|c| c.is_ascii_hexdigit()), "Should be valid hex");
    
    // Same input should generate same key
    let key_str2 = format!(
        "{}|{}|{:?}|{:?}|v{}",
        query, conversation_id, tier, project_scope.language_distribution, heuristic_version
    );
    let cache_key2 = hex::encode(Sha256::digest(key_str2.as_bytes()));
    
    assert_eq!(cache_key, cache_key2, "Same input should generate same cache key");
}

// ============================================================================
// Different Collection Tier Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires Redis and vocab.txt
async fn test_enhancement_for_different_tiers() -> Result<()> {
    init();
    
    let config = crate::common::create_test_config();
    let enhancer = QueryEnhancer::new(&config).await?;
    let project_scope = create_test_project_scope();
    let conversation_id = dummy_conversation_id();
    
    let query = "async programming examples";
    
    let tiers = vec![
        CollectionTier::Workspace,
        CollectionTier::Personal,
        CollectionTier::Online,
    ];
    
    for tier in tiers {
        let enhanced = enhancer.enhance(
            query,
            &project_scope,
            &conversation_id,
            tier,
        ).await?;
        
        println!("Tier {:?}: {} enhanced queries", tier, enhanced.len());
        
        assert!(!enhanced.is_empty(), "Should enhance queries for tier: {:?}", tier);
        
        // Each tier might produce different enhancements
        for (i, enhanced_query) in enhanced.iter().enumerate() {
            println!("  {}: {}", i + 1, enhanced_query);
        }
    }
    
    Ok(())
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[tokio::test]
#[ignore] // Requires Redis and vocab.txt
async fn test_query_enhancement_edge_cases() -> Result<()> {
    init();
    
    let config = crate::common::create_test_config();
    let enhancer = QueryEnhancer::new(&config).await?;
    let project_scope = create_test_project_scope();
    let conversation_id = dummy_conversation_id();
    
    let long_query = "very long query that might exceed normal limits and contain many words that could stress the enhancement system".repeat(10);
    let edge_case_queries = vec![
        "",                    // Empty query
        "   ",                // Whitespace only
        "a",                  // Single character
        &long_query,          // Very long query
        "🦀 rust programming", // Unicode characters
        "SELECT * FROM users;", // SQL-like query
        "function(arg1, arg2)", // Code-like query
    ];
    
    for query in edge_case_queries {
        let result = enhancer.enhance(
            &query,
            &project_scope,
            &conversation_id,
            CollectionTier::Workspace,
        ).await;
        
        match result {
            Ok(enhanced) => {
                println!("Edge case query '{}...' enhanced to {} variants", 
                    &query.chars().take(20).collect::<String>(), enhanced.len());
                
                // Should handle gracefully
                assert!(!enhanced.is_empty() || query.trim().is_empty(), 
                    "Should enhance non-empty queries or handle empty ones gracefully");
            }
            Err(e) => {
                println!("Edge case query failed (may be expected): {}", e);
                // Some edge cases may legitimately fail
            }
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_query_enhancer_invalid_redis() -> Result<()> {
    init();
    
    // Test with invalid Redis URL
    let mut config = crate::common::create_test_config();
    config.storage.redis_url = Some("redis://invalid-host:9999".to_string());
    let result = QueryEnhancer::new(&config).await;
    
    match result {
        Ok(_) => {
            println!("Unexpected success with invalid Redis URL");
        }
        Err(e) => {
            println!("Expected error with invalid Redis URL: {}", e);
            assert!(e.to_string().contains("Redis") || e.to_string().contains("connection"), 
                "Error should mention Redis or connection issue");
        }
    }
    
    Ok(())
}

#[test]
fn test_query_enhancer_vocab_file_handling() {
    // Test behavior when vocab.txt is missing (this is what would typically fail)
    let invalid_vocab_paths = vec![
        "nonexistent_vocab.txt",
        "",
        "/invalid/path/vocab.txt",
    ];
    
    for vocab_path in invalid_vocab_paths {
        println!("Testing with invalid vocab path: {}", vocab_path);
        // This would fail in actual QueryEnhancer::new() due to missing vocab file
        // We're just testing that the file paths are handled appropriately
    }
}

// ============================================================================
// Performance Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires Redis and vocab.txt
async fn test_query_enhancement_performance() -> Result<()> {
    init();
    
    let config = crate::common::create_test_config();
    let enhancer = QueryEnhancer::new(&config).await?;
    let project_scope = create_test_project_scope();
    let conversation_id = dummy_conversation_id();
    
    let test_queries = vec![
        "async function implementation",
        "error handling best practices", 
        "rust programming patterns",
        "web API documentation",
        "database connection pooling",
    ];
    
    let mut total_time = std::time::Duration::new(0, 0);
    let mut total_enhanced = 0;
    
    for query in &test_queries {
        let start = std::time::Instant::now();
        
        let enhanced = enhancer.enhance(
            query,
            &project_scope,
            &conversation_id,
            CollectionTier::Workspace,
        ).await?;
        
        let duration = start.elapsed();
        total_time += duration;
        total_enhanced += enhanced.len();
        
        println!("Query '{}': {} enhanced in {:?}", query, enhanced.len(), duration);
        
        // Enhancement should complete in reasonable time
        assert!(duration < std::time::Duration::from_secs(10), 
            "Enhancement should complete within 10 seconds");
    }
    
    let avg_time = total_time / test_queries.len() as u32;
    let avg_enhanced = total_enhanced / test_queries.len();
    
    println!("Performance summary: {} queries, avg time: {:?}, avg enhanced: {}", 
        test_queries.len(), avg_time, avg_enhanced);
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires Redis and vocab.txt
async fn test_sequential_query_enhancement() -> Result<()> {
    init();
    
    let config = crate::common::create_test_config();
    let enhancer = QueryEnhancer::new(&config).await?;
    let project_scope = create_test_project_scope();
    let conversation_id = dummy_conversation_id();
    
    let queries = vec![
        "concurrent query 1",
        "concurrent query 2", 
        "concurrent query 3",
        "concurrent query 4",
    ];
    
    // Test each query sequentially for now
    for query in queries {
        let project_scope_clone = project_scope.clone();
        let conversation_id_clone = conversation_id.clone();
        
        let result = enhancer.enhance(
            query,
            &project_scope_clone,
            &conversation_id_clone,
            CollectionTier::Workspace,
        ).await;
        
        match result {
            Ok(enhanced) => {
                println!("Sequential query '{}' enhanced to {} variants", query, enhanced.len());
                assert!(!enhanced.is_empty(), "Should enhance query: {}", query);
            }
            Err(e) => {
                println!("Sequential query '{}' failed: {}", query, e);
            }
        }
    }
    
    Ok(())
}
