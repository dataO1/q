use anyhow::{Context, Result};
use async_recursion::async_recursion;
use common::git;
use common::types::{Language, ProjectScope};
use indexing::classifier;
use std::path::PathBuf;
use tokio::fs;
use tokio::io;

/// Manages context state and project scope within the RAG system
pub struct ContextManager {}

impl ContextManager {
    /// Async constructor
    pub async fn new() -> Result<Self> {
        Ok(Self {})
    }

    /// Detects project scope including git root and language distribution asynchronously.
    #[async_recursion]
    pub async fn detect_project_scope(&self, start_path: Option<PathBuf>) -> Result<ProjectScope> {
        // Start path or cwd fallback
        let path = match start_path {
            Some(p) => p,
            None => std::env::current_dir().context("Failed to get current directory")?,
        };

        // Detect git root asynchronously via common::git helpers
        let git_root = git::find_git_root(&path).await.context("Git root detection failed")?;

        let root_path = git_root.unwrap_or(path);

        // Language distribution detection through indexing classifier
        let language_distribution = classifier::detect_language(&root_path).await;

        Ok(ProjectScope {
            root_path: root_path.to_string_lossy().to_string(),
            language_distribution,
        })
    }

    /// Calculates token budget for retrieval given conversation history size
    pub fn calculate_token_budget(&self, history_size: usize) -> usize {
        const MAX_TOKENS: usize = 4000;
        if history_size >= MAX_TOKENS {
            0
        } else {
            MAX_TOKENS - history_size
        }
    }
}
