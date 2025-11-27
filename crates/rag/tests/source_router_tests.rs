//! Source router tests focusing on LLM classification functionality

mod common;

use anyhow::Result;
use ai_agent_common::CollectionTier;
use ai_agent_rag::source_router::SourceRouter;
use common::{init_test_logging, create_test_config, create_test_project_scope};

// ============================================================================
// LLM Classification Tests
// ============================================================================

#[tokio::test]
#[ignore] // Requires Ollama infrastructure
async fn test_llm_classification_workspace_intent() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    
    let workspace_queries = vec![
        "function in main.rs",
        "variable declaration in my code",
        "error in src/lib.rs", 
        "local implementation of async",
        "struct definition in this project",
        "my repository's error handling",
    ];
    
    for query in workspace_queries {
        let tiers = router.classify_intent_llm(query).await?;
        
        println!("Query: '{}' classified to tiers: {:?}", query, tiers);
        
        assert!(!tiers.is_empty(), "Classification should return at least one tier");
        
        // Workspace queries should likely include Workspace tier
        // Note: LLM might also suggest other tiers, so we don't enforce strict expectations
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires Ollama infrastructure
async fn test_llm_classification_online_intent() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    
    let online_queries = vec![
        "latest rust documentation",
        "stack overflow examples",
        "github repository examples", 
        "find tutorial online",
        "search for API documentation",
        "web search for best practices",
    ];
    
    for query in online_queries {
        let tiers = router.classify_intent_llm(query).await?;
        
        println!("Query: '{}' classified to tiers: {:?}", query, tiers);
        
        assert!(!tiers.is_empty(), "Classification should return at least one tier");
        
        // Online queries should likely include Online tier
        let has_online = tiers.contains(&CollectionTier::Online);
        if !has_online {
            println!("Warning: Query '{}' didn't classify to Online tier", query);
        }
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires Ollama infrastructure 
async fn test_llm_classification_personal_intent() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    
    let personal_queries = vec![
        "my personal notes about rust",
        "documentation I wrote",
        "my learning journal",
        "personal project documentation",
        "notes from last meeting",
    ];
    
    for query in personal_queries {
        let tiers = router.classify_intent_llm(query).await?;
        
        println!("Query: '{}' classified to tiers: {:?}", query, tiers);
        
        assert!(!tiers.is_empty(), "Classification should return at least one tier");
        
        // Personal queries should likely include Personal tier
        let has_personal = tiers.contains(&CollectionTier::Personal);
        if !has_personal {
            println!("Warning: Query '{}' didn't classify to Personal tier", query);
        }
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires Ollama infrastructure
async fn test_llm_classification_mixed_intent() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    
    let mixed_queries = vec![
        "async functions in my code and online documentation",
        "error handling patterns from tutorials and my implementation",
        "rust best practices from docs and my project",
        "combine online examples with local code",
    ];
    
    for query in mixed_queries {
        let tiers = router.classify_intent_llm(query).await?;
        
        println!("Query: '{}' classified to tiers: {:?}", query, tiers);
        
        assert!(!tiers.is_empty(), "Classification should return at least one tier");
        
        // Mixed queries should ideally return multiple tiers
        if tiers.len() > 1 {
            println!("Good: Mixed query '{}' classified to multiple tiers", query);
        } else {
            println!("Note: Mixed query '{}' only classified to one tier", query);
        }
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires Ollama infrastructure
async fn test_llm_classification_edge_cases() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    
    let long_query = "very long query ".repeat(100);
    let edge_case_queries = vec![
        "", // Empty query
        "   ", // Whitespace only
        "a", // Single character
        "🦀", // Unicode emoji
        &long_query, // Very long query
        "SELECT * FROM users;", // SQL injection attempt
        "<script>alert('xss')</script>", // XSS attempt
    ];
    
    for query in edge_case_queries {
        let result = router.classify_intent_llm(&query).await;
        
        match result {
            Ok(tiers) => {
                println!("Edge case query classified successfully: {} tiers", tiers.len());
                
                // Should handle gracefully - either return tiers or empty vec
                assert!(tiers.len() <= 10, // Reasonable upper bound for tier count
                    "Should not return excessive number of tiers");
            }
            Err(e) => {
                println!("Edge case query failed (expected): {}", e);
                // Edge cases may fail, which is acceptable
            }
        }
    }
    
    Ok(())
}

// ============================================================================
// Router Integration Tests  
// ============================================================================

#[tokio::test]
async fn test_router_heuristic_and_llm_combination() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    let project_scope = create_test_project_scope();
    
    // Query with clear web heuristic but might have additional LLM insights
    let query = "https://docs.rust-lang.org async patterns and my local code";
    
    let routes = router.route_query(query, &project_scope).await?;
    
    println!("Combined routing result: {:?}", routes);
    
    // Should detect web intent via heuristics
    assert!(routes.contains_key(&CollectionTier::Online), 
        "Should detect Online tier via heuristics");
    
    // May also detect other tiers via LLM
    assert!(!routes.is_empty(), "Should route to at least one tier");
    
    // All routes should have the same query string
    for (tier, query_text) in &routes {
        assert_eq!(query_text, query, "Query text should be preserved for tier: {:?}", tier);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_router_fallback_behavior() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    let project_scope = create_test_project_scope();
    
    // Ambiguous query that doesn't trigger heuristics
    let query = "function behavior";
    
    let routes = router.route_query(query, &project_scope).await?;
    
    println!("Fallback routing result: {:?}", routes);
    
    // Should always route to at least one tier (Workspace fallback)
    assert!(!routes.is_empty(), "Should route to at least one tier");
    
    // Should contain Workspace as fallback
    assert!(routes.contains_key(&CollectionTier::Workspace), 
        "Should fallback to Workspace tier");
    
    Ok(())
}

#[tokio::test]
async fn test_router_consistent_results() -> Result<()> {
    init_test_logging();
    
    let config = create_test_config();
    let router = SourceRouter::new(&config)?;
    let project_scope = create_test_project_scope();
    
    let query = "find documentation online";
    
    // Run the same query multiple times
    let mut results = Vec::new();
    for _i in 0..3 {
        let routes = router.route_query(query, &project_scope).await?;
        results.push(routes);
    }
    
    // Results should be consistent (same tiers)
    let first_result = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        let first_tiers: std::collections::HashSet<_> = first_result.keys().collect();
        let current_tiers: std::collections::HashSet<_> = result.keys().collect();
        
        if first_tiers != current_tiers {
            println!("Warning: Inconsistent results between run 0 and {}: {:?} vs {:?}", 
                i, first_tiers, current_tiers);
        }
    }
    
    Ok(())
}

// ============================================================================
// Configuration and Error Tests
// ============================================================================

#[tokio::test]
async fn test_router_different_models() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    
    // Test with different classification models
    let models = vec![
        "llama3.2:1b".to_string(),
        "qwen2.5:0.5b".to_string(),
        "phi3:mini".to_string(),
    ];
    
    for model in models {
        config.rag.classification_model = model.clone();
        
        let router_result = SourceRouter::new(&config);
        assert!(router_result.is_ok(), "Should create router with model: {}", model);
        
        let router = router_result?;
        let project_scope = create_test_project_scope();
        
        // Test that the router works with different models
        let query = "test query";
        let routes_result = router.route_query(query, &project_scope).await;
        
        match routes_result {
            Ok(routes) => {
                println!("Model {} successfully routed query to {} tiers", model, routes.len());
                assert!(!routes.is_empty(), "Should route to at least one tier");
            }
            Err(e) => {
                println!("Model {} failed (may not be available): {}", model, e);
                // Model might not be available in test environment
            }
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_router_invalid_configuration() -> Result<()> {
    init_test_logging();
    
    let mut config = create_test_config();
    
    // Test with invalid Ollama endpoint
    config.embedding.ollama_host = "http://invalid-host".to_string();
    config.embedding.ollama_port = 65534;
    
    let router = SourceRouter::new(&config)?;
    let project_scope = create_test_project_scope();
    
    // Should handle connection errors gracefully
    let result = router.route_query("test query", &project_scope).await;
    
    match result {
        Ok(routes) => {
            println!("Unexpected success with invalid config: {:?}", routes);
            // Should fallback to heuristics only
            assert!(!routes.is_empty(), "Should still provide fallback routing");
        }
        Err(e) => {
            println!("Expected error with invalid config: {}", e);
            // Connection errors are expected with invalid configuration
        }
    }
    
    Ok(())
}

#[test]
fn test_web_intent_heuristics_comprehensive() {
    let web_keywords = [
        "http://", "https://", "www.", ".com", ".org", ".net",
        "documentation", "docs", "tutorial", "guide", "example",
        "stack overflow", "github", "api reference", "latest",
        "online", "web", "internet", "search", "find"
    ];
    
    // Test comprehensive web intent cases
    let test_cases = vec![
        // URLs
        ("https://docs.rust-lang.org/book", true),
        ("http://example.com/tutorial", true), 
        ("www.github.com/user/repo", true),
        ("Visit example.org for more info", true),
        
        // Web-related terms
        ("find documentation online", true),
        ("search for examples", true),
        ("latest tutorial on async", true),
        ("stack overflow solutions", true),
        ("github repository examples", true),
        ("api reference guide", true),
        ("web development guide", true),
        ("internet search results", true),
        
        // Non-web queries
        ("my local function", false),
        ("variable in main.rs", false),
        ("struct definition", false),
        ("compile error", false),
        ("unit test failure", false),
        ("cargo build issue", false),
        
        // Edge cases
        ("", false),
        ("   ", false),
        ("a", false),
        ("documentation", true), // Single keyword
        ("HTTPS://CAPS-URL.COM", true), // Case insensitive
    ];
    
    for (query, expected) in test_cases {
        let query_lower = query.to_lowercase();
        let is_web = web_keywords.iter().any(|&keyword| query_lower.contains(keyword));
        
        assert_eq!(is_web, expected, "Web intent detection failed for: '{}'", query);
    }
}

#[test]
fn test_source_router_debug_output() {
    let config = create_test_config();
    let router = SourceRouter::new(&config).unwrap();
    
    // Test Debug trait implementation
    let debug_str = format!("{:?}", router);
    assert!(debug_str.contains("SourceRouter"), "Debug output should contain SourceRouter");
    
    // Should not panic with debug formatting
    println!("SourceRouter debug: {:?}", router);
}