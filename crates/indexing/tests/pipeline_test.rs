use ai_agent_indexing::pipeline::{IndexingPipeline, IndexingCoordinator};
use ai_agent_common::config::*;
use ai_agent_common::types::CollectionTier;
use ai_agent_storage::QdrantClient;
use tempfile::TempDir;
use std::fs;
use std::path::PathBuf;

/// Helper to create test configuration pointing to test services
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
            redis_url: Some("redis://localhost:16379".to_string()),
        },
    }
}

/// Helper to cleanup test collections
async fn cleanup_collection(qdrant_url: &str, collection: &str) {
    let client = QdrantClient::new(qdrant_url).ok();
    if let Some(client) = client {

        client.delete_collection(collection).await.ok();
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
fn test_is_code_file_detection() {
    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    // Code files
    assert!(pipeline.is_code_file(&PathBuf::from("main.rs")));
    assert!(pipeline.is_code_file(&PathBuf::from("app.py")));
    assert!(pipeline.is_code_file(&PathBuf::from("script.js")));
    assert!(pipeline.is_code_file(&PathBuf::from("component.tsx")));
    assert!(pipeline.is_code_file(&PathBuf::from("main.go")));
    assert!(pipeline.is_code_file(&PathBuf::from("App.java")));

    // Non-code files
    assert!(!pipeline.is_code_file(&PathBuf::from("README.md")));
    assert!(!pipeline.is_code_file(&PathBuf::from("notes.txt")));
    assert!(!pipeline.is_code_file(&PathBuf::from("data.json")));
    assert!(!pipeline.is_code_file(&PathBuf::from("config.toml")));
}

// ============================================================================
// Integration Tests (Require Ollama + Test Qdrant)
// ============================================================================

#[tokio::test]
#[ignore] // Run with: cargo test --test pipeline_test -- --ignored
async fn test_index_single_rust_file() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.rs");

    let rust_code = r#"
/// Main entry point
fn main() {
    println!("Hello, world!");
}

/// Helper function for calculations
fn helper(x: i32, y: i32) -> i32 {
    x + y
}

/// A simple struct
struct MyStruct {
    field: i32,
}

impl MyStruct {
    /// Constructor
    fn new() -> Self {
        Self { field: 0 }
    }

    /// Get the field value
    fn get_field(&self) -> i32 {
        self.field
    }
}
"#;

    fs::write(&test_file, rust_code).unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    // Index the file
    let result = pipeline.index_file(&test_file, CollectionTier::Workspace).await;

    assert!(result.is_ok(), "Should index Rust file successfully: {:?}", result.err());

    // Verify in Qdrant
    let qdrant = QdrantClient::new(&config.storage.qdrant_url).unwrap();
    let collection_name = CollectionTier::Workspace.collection_name();

    // Small delay for indexing to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Check collection exists
    let exists = qdrant.collection_exists(&collection_name).await.unwrap();
    assert!(exists, "Collection should exist after indexing");

    // Cleanup
    cleanup_collection(&config.storage.qdrant_url, &collection_name).await;
}

#[tokio::test]
#[ignore]
async fn test_index_python_file() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("app.py");

    let python_code = r#"
"""Main application module"""

def main():
    """Main function"""
    print("Hello, world!")

class Calculator:
    """A simple calculator class"""

    def __init__(self):
        """Initialize calculator"""
        self.result = 0

    def add(self, x, y):
        """Add two numbers"""
        self.result = x + y
        return self.result

    def multiply(self, x, y):
        """Multiply two numbers"""
        self.result = x * y
        return self.result

if __name__ == "__main__":
    main()
"#;

    fs::write(&test_file, python_code).unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    let result = pipeline.index_file(&test_file, CollectionTier::Workspace).await;
    assert!(result.is_ok(), "Should index Python file successfully: {:?}", result.err());

    // Cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    let collection_name = CollectionTier::Workspace.collection_name();
    cleanup_collection(&config.storage.qdrant_url, &collection_name).await;
}

#[tokio::test]
#[ignore]
async fn test_index_javascript_file() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("app.js");

    let js_code = r#"
/**
 * Main application entry point
 */
function main() {
    console.log("Hello, world!");
}

/**
 * Calculator class
 */
class Calculator {
    constructor() {
        this.result = 0;
    }

    /**
     * Add two numbers
     */
    add(x, y) {
        this.result = x + y;
        return this.result;
    }

    /**
     * Multiply two numbers
     */
    multiply(x, y) {
        this.result = x * y;
        return this.result;
    }
}

export { main, Calculator };
"#;

    fs::write(&test_file, js_code).unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    let result = pipeline.index_file(&test_file, CollectionTier::Workspace).await;
    assert!(result.is_ok(), "Should index JavaScript file successfully: {:?}", result.err());

    // Cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    let collection_name = CollectionTier::Workspace.collection_name();
    cleanup_collection(&config.storage.qdrant_url, &collection_name).await;
}

#[tokio::test]
#[ignore]
async fn test_index_markdown_file() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("README.md");

    let markdown = r#"
# Project Documentation

This is a comprehensive test project for the AI agent system.

## Overview

The system provides intelligent code analysis and retrieval.

## Features

- Semantic code search
- Context-aware suggestions
- Multi-language support
- Real-time indexing

## Usage
fn main() {
println!("Example");
}


## Installation

1. Install dependencies
2. Configure the system
3. Run the indexer

## License

MIT License - see LICENSE file for details.
"#;

    fs::write(&test_file, markdown).unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    let result = pipeline.index_file(&test_file, CollectionTier::Personal).await;
    assert!(result.is_ok(), "Should index Markdown file successfully: {:?}", result.err());

    // Cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    let collection_name = CollectionTier::Personal.collection_name();
    cleanup_collection(&config.storage.qdrant_url, &collection_name).await;
}

#[tokio::test]
#[ignore]
async fn test_index_directory() {
    let temp = TempDir::new().unwrap();

    // Create multiple files
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src/main.rs"), "fn main() { println!(\"test\"); }").unwrap();
    fs::write(temp.path().join("src/lib.rs"), "pub fn test() { println!(\"lib\"); }").unwrap();
    fs::write(temp.path().join("README.md"), "# Test Project\n\nThis is a test.").unwrap();
    fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    let result = pipeline
        .index_directory(temp.path(), CollectionTier::Workspace, &["rs", "md"])
        .await;

    assert!(result.is_ok(), "Should index directory successfully: {:?}", result.err());

    // Cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    let collection_name = CollectionTier::Workspace.collection_name();
    cleanup_collection(&config.storage.qdrant_url, &collection_name).await;
}

#[tokio::test]
#[ignore]
async fn test_batch_indexing() {
    let temp = TempDir::new().unwrap();

    let file1 = temp.path().join("file1.rs");
    let file2 = temp.path().join("file2.py");
    let file3 = temp.path().join("file3.js");

    fs::write(&file1, "fn test1() { println!(\"1\"); }").unwrap();
    fs::write(&file2, "def test2():\n    print('2')").unwrap();
    fs::write(&file3, "function test3() { console.log('3'); }").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    let files = vec![
        (file1, CollectionTier::Workspace),
        (file2, CollectionTier::Workspace),
        (file3, CollectionTier::Workspace),
    ];

    let results = pipeline.index_batch(files).await.unwrap();

    assert_eq!(results.len(), 3);
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, 3, "All files should index successfully");

    // Cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    let collection_name = CollectionTier::Workspace.collection_name();
    cleanup_collection(&config.storage.qdrant_url, &collection_name).await;
}

#[tokio::test]
#[ignore]
async fn test_tier_separation() {
    let temp = TempDir::new().unwrap();

    let workspace_file = temp.path().join("workspace.rs");
    let personal_file = temp.path().join("personal.md");
    let system_file = temp.path().join("system.txt");

    fs::write(&workspace_file, "fn workspace() { println!(\"work\"); }").unwrap();
    fs::write(&personal_file, "# Personal Notes\n\nMy thoughts.").unwrap();
    fs::write(&system_file, "System documentation content").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    // Index into different tiers
    pipeline.index_file(&workspace_file, CollectionTier::Workspace).await.unwrap();
    pipeline.index_file(&personal_file, CollectionTier::Personal).await.unwrap();
    pipeline.index_file(&system_file, CollectionTier::System).await.unwrap();

    // Verify collections exist
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let qdrant = QdrantClient::new(&config.storage.qdrant_url).unwrap();

    let workspace_exists = qdrant.collection_exists(&CollectionTier::Workspace.collection_name()).await.unwrap();
    let personal_exists = qdrant.collection_exists(&CollectionTier::Personal.collection_name()).await.unwrap();
    let system_exists = qdrant.collection_exists(&CollectionTier::System.collection_name()).await.unwrap();

    assert!(workspace_exists, "Workspace collection should exist");
    assert!(personal_exists, "Personal collection should exist");
    assert!(system_exists, "System collection should exist");

    // Cleanup all tiers
    cleanup_collection(&config.storage.qdrant_url, &CollectionTier::Workspace.collection_name()).await;
    cleanup_collection(&config.storage.qdrant_url, &CollectionTier::Personal.collection_name()).await;
    cleanup_collection(&config.storage.qdrant_url, &CollectionTier::System.collection_name()).await;
}

#[tokio::test]
#[ignore]
async fn test_reindex_file() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.rs");

    // Initial content
    fs::write(&test_file, "fn main() { println!(\"v1\"); }").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();
    let qdrant = QdrantClient::new(&config.storage.qdrant_url).unwrap();

    // First index
    pipeline.index_file(&test_file, CollectionTier::Workspace).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update content
    fs::write(&test_file, "fn main() { println!(\"v2 - updated\"); }").unwrap();

    // Reindex
    let result = pipeline.reindex_file(&test_file, CollectionTier::Workspace, &qdrant).await;
    assert!(result.is_ok(), "Reindexing should succeed: {:?}", result.err());

    // Cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    cleanup_collection(&config.storage.qdrant_url, &CollectionTier::Workspace.collection_name()).await;
}

#[tokio::test]
#[ignore]
async fn test_empty_file() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("empty.rs");

    fs::write(&test_file, "").unwrap();

    let config = test_config();
    let pipeline = IndexingPipeline::new(&config).unwrap();

    let result = pipeline.index_file(&test_file, CollectionTier::Workspace).await;
    // Empty file should either succeed or fail gracefully
    println!("Empty file result: {:?}", result);

    // Cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    cleanup_collection(&config.storage.qdrant_url, &CollectionTier::Workspace.collection_name()).await;
}
