use anyhow::Result;
use ai_agent_common::{CollectionTier, ProjectScope};
use futures::StreamExt;
use retriever::{MultiSourceRetriever};
use std::collections::HashMap;

#[tokio::test]
async fn test_multisource_retriever_stream() -> Result<()> {
    let retriever = MultiSourceRetriever::new("http://localhost:6333").await?;

    let queries = vec![
        (CollectionTier::Code, "async function".to_string()),
        (CollectionTier::Docs, "error handling".to_string()),
    ];

    let project_scope = ProjectScope { language_distribution: vec!["en".to_string()] };

    let mut stream = retriever.retrieve_stream(queries, &project_scope);

    let mut count = 0;
    while let Some(res) = stream.next().await {
        let fragment = res?;
        println!("Got fragment: {:?}", fragment.summary);
        count += 1;
        if count > 10 {
            break;
        }
    }

    assert!(count > 0);

    Ok(())
}
