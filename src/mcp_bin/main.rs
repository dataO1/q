use anyhow::Result;
use semantic_search::Config;
use std::path::PathBuf;

/// MCP Server for semantic code search
///
/// This is a stub implementation. To complete it:
/// 1. Add `mcp_sdk_rs` to Cargo.toml when available
/// 2. Implement MCP tool definitions for:
///    - search_code: Perform semantic search
///    - list_indexed_files: Show what's indexed
///    - get_context: Fetch full file context
/// 3. Set up stdio transport for Claude Desktop integration
///
/// Expected tools to expose:
/// - search_code(query: String, limit?: number) -> [{path, score, snippet}]
/// - list_indexed_files() -> [String]
/// - get_file_context(path: String) -> String
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = if PathBuf::from("config.toml").exists() {
        Config::from_file(&PathBuf::from("config.toml"))?
    } else {
        Config::default_config()
    };

    tracing::info!("MCP Server starting...");
    tracing::info!("Connected to Ollama at: {}", config.ollama.url);
    tracing::info!("Connected to Qdrant at: {}", config.qdrant.url);
    tracing::info!(
        "Collection: {}",
        config.qdrant.collection_name
    );

    // TODO: Initialize MCP server
    // let mut server = mcp_sdk_rs::server::Server::new(config);
    //
    // server.add_tool(Tool {
    //     name: "search_code".to_string(),
    //     description: "Search code semantically".to_string(),
    //     input_schema: json!({...}),
    // });
    //
    // server.run().await?;

    tracing::warn!("MCP Server is in stub mode. Waiting indefinitely.");
    tokio::signal::ctrl_c().await?;

    Ok(())
}
