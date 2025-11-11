// src/bin/rag_cli.rs
use anyhow::Result;
use std::env;
use std::io::{self, Write};

use ai_agent_common::{AgentContext, CollectionTier, ProjectScope};
use futures::StreamExt;
use retriever::MultiSourceRetriever;

#[tokio::main]
async fn main() -> Result<()> {

    let config = SystemConfig::from_file(config_path.to_str().unwrap()).unwrap();
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

    // Create a simple AgentContext with example languages and file types
    let agent_ctx = AgentContext {
        project_root: cwd.clone(),
        languages: vec!["rust".to_string(), "python".to_string()],
        file_types: vec!["source".to_string(), "docs".to_string()],
    };

    // Example ProjectScope
    let project_scope = ProjectScope {
        language_distribution: vec!["rust".to_string()], // example distribution
    };

    // Create retriever and run query
    let qdrant_url = "http://localhost:6333"; // adjust as needed
    let rag = SmartMultiSourceRag::new(qdrant_url).await?;

    let queries = vec![(CollectionTier::Code, query)];

    let mut stream = rag.retrieve_stream(queries, &project_scope, &agent_ctx);

    // Stream results in batches and print summaries incrementally
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    while let Some(batch_result) = stream.next().await {
        match batch_result {
            Ok(batch) => {
                writeln!(handle, "=== Batch ({} results) ===", batch.len())?;
                for fragment in batch {
                    writeln!(handle, "Summary: {}", fragment.summary)?;
                    writeln!(handle, "Content preview: {:.100}", fragment.content)?;
                    writeln!(handle, "Source: {}", fragment.source)?;
                    writeln!(handle, "Score: {:.4}", fragment.score)?;
                    writeln!(handle, "------------")?;
                    handle.flush()?;
                }
            }
            Err(e) => {
                eprintln!("Error streaming results: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}
