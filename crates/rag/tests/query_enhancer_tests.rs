use anyhow::Result;
use ai_agent_common::{ProjectScope, ConversationId, Language};
use std::collections::HashMap;
use std::sync::Once;
use ai_agent_rag::query_enhancer::QueryEnhancer;

static INIT: Once = Once::new();

fn init() {
    INIT.call_once(|| {
        // Initialize logging or setup once if needed
        let _ = env_logger::builder().is_test(true).try_init();
    });
}

fn dummy_project_scope() -> ProjectScope {
    ProjectScope::new(
        "/test/project".to_string(),
        Some(std::path::PathBuf::from("/test/project/src/main.rs")),
        vec![(Language::Rust, 1.0)]
    )
}

fn dummy_conversation_id() -> ConversationId {
    ConversationId("test_conversation".to_string())
}

#[tokio::test]
async fn test_query_enhancer_simple() -> Result<()> {
    init();
    let qe = QueryEnhancer::new("redis://localhost/")?;
    let sources = [("qdrant", "Vector DB with code snippets"), ("web", "Web crawl dataset")];

    let results = qe.enhance_for_sources(
        "find async API docs",
        &dummy_project_scope(),
        &dummy_conversation_id(),
        &sources,
    ).await?;

    assert!(results.contains_key("qdrant"));
    assert!(results.contains_key("web"));

    for (source, enhanced_query) in results.iter() {
        println!("Source: {}, Enhanced Query: {}", source, enhanced_query);
        assert!(enhanced_query.len() > 0);
    }

    // Test cache effectiveness by calling again and expecting no error
    let results_cached = qe.enhance_for_sources(
        "find async API docs",
        &dummy_project_scope(),
        &dummy_conversation_id(),
        &sources,
    ).await?;

    assert_eq!(results, results_cached);

    Ok(())
}
