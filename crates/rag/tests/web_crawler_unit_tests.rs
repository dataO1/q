//! Web crawler unit tests that don't require external infrastructure

use anyhow::Result;
use ai_agent_common::SystemConfig;

#[test]
fn test_web_crawler_config_creation() {
    let config = SystemConfig::default();
    
    // Should have web crawler config with default values
    assert_eq!(config.rag.web_crawler.enabled, true);
    assert_eq!(config.rag.web_crawler.max_urls_per_query, 5);
    assert_eq!(config.rag.web_crawler.chunk_size, 1024);
    assert_eq!(config.rag.web_crawler.chunk_overlap, 100);
    assert_eq!(config.rag.web_crawler.respect_robots_txt, true);
    
    // Should have default collection names
    assert_eq!(config.rag.web_crawler.web_content_collection, "web_content");
    assert_eq!(config.rag.web_crawler.web_query_cache_collection, "web_query_cache");
    assert_eq!(config.rag.web_crawler.content_cache_prefix, "web_content_cache:");
    assert_eq!(config.rag.web_crawler.query_cache_prefix, "web_query_cache:");
}

#[test]
fn test_url_validation() -> Result<()> {
    use url::Url;
    
    // Test valid URLs
    let valid_urls = vec![
        "https://docs.rust-lang.org/book/",
        "http://localhost:8080/api",
        "https://github.com/user/repo",
        "https://stackoverflow.com/questions/123",
    ];
    
    for url_str in valid_urls {
        let url = Url::parse(url_str);
        assert!(url.is_ok(), "URL should be valid: {}", url_str);
        
        let url = url.unwrap();
        assert!(url.host_str().is_some(), "URL should have host: {}", url_str);
    }
    
    // Test invalid URLs
    let invalid_urls = vec![
        "not-a-url",
        "https://",
        "",
        "://missing-scheme",
        "https://[invalid-host]",
    ];
    
    for url_str in invalid_urls {
        let url = Url::parse(url_str);
        assert!(url.is_err(), "URL should be invalid: {}", url_str);
    }
    
    Ok(())
}

#[test]
fn test_content_hashing() {
    use sha2::{Digest, Sha256};
    
    let content1 = "This is test content for hashing";
    let content2 = "This is test content for hashing"; // Same
    let content3 = "This is different content";
    
    let hash1 = format!("{:x}", Sha256::digest(content1.as_bytes()));
    let hash2 = format!("{:x}", Sha256::digest(content2.as_bytes()));
    let hash3 = format!("{:x}", Sha256::digest(content3.as_bytes()));
    
    // Same content should have same hash
    assert_eq!(hash1, hash2, "Identical content should have same hash");
    
    // Different content should have different hash
    assert_ne!(hash1, hash3, "Different content should have different hash");
    
    // Hash should be consistent
    let hash1_repeat = format!("{:x}", Sha256::digest(content1.as_bytes()));
    assert_eq!(hash1, hash1_repeat, "Hash should be consistent");
}

#[test]
fn test_chunking_logic() {
    let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10";
    let lines: Vec<&str> = content.lines().collect();
    
    let chunk_size = 3;
    let overlap = 1;
    
    let mut chunks = Vec::new();
    let mut current_pos = 0;
    
    while current_pos < lines.len() {
        let end_pos = std::cmp::min(current_pos + chunk_size, lines.len());
        let chunk_lines = &lines[current_pos..end_pos];
        let chunk_content = chunk_lines.join("\n");
        
        if !chunk_content.trim().is_empty() {
            chunks.push(chunk_content);
        }
        
        if end_pos >= lines.len() {
            break;
        }
        current_pos = end_pos - overlap.min(end_pos);
    }
    
    // Should create multiple chunks with overlap
    assert!(chunks.len() > 1, "Should create multiple chunks");
    
    // Check overlap exists
    let first_chunk = &chunks[0];
    let second_chunk = &chunks[1];
    
    println!("Chunk 1: {}", first_chunk);
    println!("Chunk 2: {}", second_chunk);
    
    // Should have some overlapping content (Line 3 should appear in both)
    assert!(first_chunk.contains("Line 3"), "First chunk should contain Line 3");
    assert!(second_chunk.contains("Line 3"), "Second chunk should contain Line 3 (overlap)");
}

#[test] 
fn test_web_intent_heuristics() {
    let web_keywords = [
        "http://", "https://", "www.", ".com", ".org", ".net",
        "documentation", "docs", "tutorial", "guide", "example",
        "stack overflow", "github", "api reference", "latest",
        "online", "web", "internet", "search", "find"
    ];
    
    let web_queries = vec![
        "https://docs.rust-lang.org",
        "find documentation online",
        "search for examples",
        "www.example.com tutorial",
        "latest guide on github",
        "stack overflow examples",
    ];
    
    let non_web_queries = vec![
        "my local code",
        "function in this project", 
        "error in main.rs",
        "variable declaration",
    ];
    
    for query in web_queries {
        let query_lower = query.to_lowercase();
        let is_web = web_keywords.iter().any(|&keyword| query_lower.contains(keyword));
        assert!(is_web, "Query '{}' should be detected as web intent", query);
    }
    
    for query in non_web_queries {
        let query_lower = query.to_lowercase();
        let is_web = web_keywords.iter().any(|&keyword| query_lower.contains(keyword));
        assert!(!is_web, "Query '{}' should NOT be detected as web intent", query);
    }
}

#[test]
fn test_cache_key_generation() {
    let base_prefix = "web_content_cache:";
    let urls = vec![
        "https://docs.rust-lang.org/book/ch01-01-installation.html",
        "https://stackoverflow.com/questions/12345",
        "https://github.com/user/repo/blob/main/README.md",
    ];
    
    let mut cache_keys = std::collections::HashSet::new();
    
    for url in urls {
        let cache_key = format!("{}{}", base_prefix, url);
        
        // Keys should be unique
        assert!(cache_keys.insert(cache_key.clone()), 
            "Cache key should be unique: {}", cache_key);
        
        // Keys should contain the URL
        assert!(cache_key.contains(url), 
            "Cache key should contain URL: {}", cache_key);
        
        // Keys should have the prefix
        assert!(cache_key.starts_with(base_prefix),
            "Cache key should start with prefix: {}", cache_key);
    }
}