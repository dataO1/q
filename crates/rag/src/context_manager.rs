use anyhow::{Context, Result};
use async_recursion::async_recursion;
use ai_agent_common::types::ProjectScope;
use ai_agent_indexing::classifier;
use repo_root::projects::GitProject;
use repo_root::{ ProjectType, ProjectTypes, RepoRoot};
use std::env::current_dir;
use std::path::PathBuf;

/// Manages context state and project scope within the RAG system
pub struct ContextManager {}

impl ContextManager {
    /// Async constructor
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    /// Detects project scope including git root and language distribution asynchronously.
    #[async_recursion]
    pub async fn detect_project_scope(&self, start_path_string: Option<String>) -> Result<ProjectScope> {
        // Start path or cwd fallback
        let path_string = match start_path_string.clone() {
            Some(p) => p,
            None => current_dir().context("Failed to get current directory")?.to_str().unwrap().to_string(),
        };

        let path = PathBuf::from(path_string.clone());

        // Detect git root asynchronously via common::git helpers
        // let repo_root = git::find_git_root(&path).await.context("Git root detection failed")?;
        let root = RepoRoot::<GitProject>::new(&path).path;


        let root_path = root.to_str().unwrap_or(&path_string).to_string();

        let current_file = if path.is_file() {Some(path)} else {None};
        // Language distribution detection through indexing classifier
        let language_distribution = classifier::detect_languages(&root).await;

        Ok(ProjectScope {
            root:root_path,
            language_distribution,
            current_file: current_file
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
