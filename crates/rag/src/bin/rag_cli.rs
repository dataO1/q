use ai_agent_common::llm::EmbeddingClient;
use ai_agent_rag::SmartMultiSourceRag;
// src/bin/rag_cli.rs
use anyhow::Result;
use std::env;
use std::io::{self, Write};

use ai_agent_common::{ConversationId, Language, ProjectScope, SystemConfig};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {

    let _ = env_logger::builder().is_test(true).try_init();
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config.dev.toml".to_string());
    let config = SystemConfig::from_file(&config_path).unwrap();
    // Parse query from CLI args
    let mut args = env::args().skip(1);
    let query = match args.next() {
        Some(q) => q,
        None => {
            eprintln!("Usage: rag_cli \"search query\"");
            std::process::exit(1);
        }
    };

    // Detect current working directory (project_root)
    let cwd = env::current_dir()?.to_string_lossy().into_owned();

    // Example ProjectScope
    let project_scope = ProjectScope::new(cwd.clone(), None, vec![(Language::Rust, 1f32)]);
    let conversation_id = ConversationId::new();

    let embedding_client = EmbeddingClient::new(&config.embedding.dense_model)?;
    let rag = SmartMultiSourceRag::new(&config, &embedding_client).await?;


    let mut stream = rag.retrieve_stream(&query, &project_scope, &conversation_id).await?;

    // Stream results in batches and print summaries incrementally
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    while let Some(batch_result) = stream.next().await {
        match batch_result {
            Ok(fragment) => {
                // writeln!(handle, "=== Batch ({} results) ===", batch.len())?;
                writeln!(handle, "Summary: {}", fragment.summary)?;
                writeln!(handle, "Content preview: {:.100}", fragment.content)?;
                writeln!(handle, "Source: {}", fragment.source)?;
                writeln!(handle, "Score: {:.4}", fragment.score)?;
                writeln!(handle, "------------")?;
                handle.flush()?;
            }
            Err(e) => {
                eprintln!("Error streaming results: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}
