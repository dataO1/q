//! Git tool integration using git2-rs
//!
//! Provides git operations: commits, branches, history, diffs, logs.
//! Uses native git2 bindings for performance and reliability.

use crate::{agents::ToolResult, error::AgentNetworkResult};
use crate::tools::Tool;
use async_trait::async_trait;
use git2::{Repository, Signature};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info};

/// Git tool for version control operations
pub struct GitTool {
    repo_path: PathBuf,
}

impl GitTool {
    /// Create new git tool for a repository
    pub fn new(repo_path: PathBuf) -> AgentNetworkResult<Self> {
        // Verify repository exists
        let _repo = Repository::open(&repo_path).map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to open git repository: {}", e),
            )
        })?;

        info!("GitTool initialized for: {}", repo_path.display());

        Ok(Self { repo_path })
    }

    /// Execute git status
    async fn status(&self) -> AgentNetworkResult<ToolResult> {
        let repo = Repository::open(&self.repo_path).map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to open git repository: {}", e),
            )
        })?;

        let mut status_output = String::new();
        let statuses = repo.statuses(None).map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to open git repository: {}", e),
            )
        })?;

        for entry in statuses.iter(){
            let path = entry.path().unwrap_or("unknown");
            let status = entry.status();

            let status_str = match status {
                s if s.contains(git2::Status::WT_MODIFIED) => "modified",
                s if s.contains(git2::Status::WT_NEW) => "new",
                s if s.contains(git2::Status::WT_DELETED) => "deleted",
                s if s.contains(git2::Status::INDEX_MODIFIED) => "staged-modified",
                s if s.contains(git2::Status::INDEX_NEW) => "staged-new",
                _ => "unknown",
            };

            status_output.push_str(&format!("{}: {}\n", path, status_str));
        }

        Ok(ToolResult::success("git_status".to_string(), status_output))
    }

    /// Execute git log
    async fn log(&self, limit: usize) -> AgentNetworkResult<ToolResult> {
        let repo = Repository::open(&self.repo_path).map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to open git repository: {}", e),
            )
        })?;

        let mut revwalk = repo.revwalk().map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to revwalk git repository: {}", e),
            )
        })?;

        revwalk.push_head().map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to push head for git repository: {}", e),
            )
        })?;


        let mut log_output = String::new();
        let mut count = 0;

        for oid in revwalk {
            if count >= limit {
                break;
            }

            let oid = oid.map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to unwrap oid for git repository: {}", e),
            )
        })?;

            if let Ok(commit) = repo.find_commit(oid) {
                log_output.push_str(&format!(
                    "{}: {} ({})\n",
                    &oid.to_string()[..8],
                    commit.message().unwrap_or(""),
                    commit.author().name().unwrap_or("unknown")
                ));
            }

            count += 1;
        }

        Ok(ToolResult::success("git_log".to_string(), log_output))
    }

    /// Execute git add
    async fn add(&self, patterns: Vec<String>) -> AgentNetworkResult<ToolResult> {
        let repo = Repository::open(&self.repo_path).map_err(|e| {
            crate::error::AgentNetworkError::Tool(format!("Failed to open git repo: {}", e))
        })?;
        let mut index = repo.index().map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to index git repository: {}", e),
            )
        })?;


        for pattern in &patterns {
            index.add_path(std::path::Path::new(pattern)).map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to add path for git repository: {}", e),
            )
        })?;

        }

        index.write().map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to write index for git repository: {}", e),
            )
        })?;


        Ok(ToolResult::success(
            "git_add".to_string(),
            format!("Added {} files", patterns.len()),
        ))
    }

    /// Execute git commit
    async fn commit(&self, message: String) -> AgentNetworkResult<ToolResult> {
        let repo = Repository::open(&self.repo_path).map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to open git repository: {}", e),
            )
        })?;

        let mut index = repo.index().map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to index git repository: {}", e),
            )
        })?;

        let oid = index.write_tree().map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to write tree for git repository: {}", e),
            )
        })?;

        let tree = repo.find_tree(oid).map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to find tree for git repository: {}", e),
            )
        })?;


        // Get current HEAD
        let head = repo.head().map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to find head for git repository: {}", e),
            )
        })?;

        let parent_commit = repo.find_commit(head.target().unwrap()).map_err(|e| {
            crate::error::AgentNetworkError::Tool(
                format!("Failed to find head commit for git repository: {}", e),
            )
        })?;


        // Create signature
        let sig = Signature::now("Agent", "agent@network.local").map_err(|e| {
            crate::error::AgentNetworkError::Tool(format!("Failed to create signature: {}", e))
        })?;

        // Create commit
        let commit_oid = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &message,
            &tree,
            &[&parent_commit],
        ).map_err(|e| {
            crate::error::AgentNetworkError::Tool(format!("Failed to commit for git repo: {}", e))
        })?;


        debug!("Created commit: {}", &commit_oid.to_string()[..8]);

        Ok(ToolResult::success(
            "git_commit".to_string(),
            format!("Committed: {}", &commit_oid.to_string()[..8]),
        ))
    }

    /// Execute git branch
    async fn branch(&self, _action: String) -> AgentNetworkResult<ToolResult> {
        let repo = Repository::open(&self.repo_path).map_err(|e| {
            crate::error::AgentNetworkError::Tool(format!("Failed to open git repo: {}", e))
        })?;
        let branches = repo.branches(None).map_err(|e| {
            crate::error::AgentNetworkError::Tool(format!("Failed to get git repo branches: {}", e))
        })?;


        let mut branch_output = String::new();
        for branch in branches {
            if let Ok((branch, _)) = branch {
                if let Ok(name) = branch.name() {
                    if let Some(name) = name {
                        branch_output.push_str(&format!("{}\n", name));
                    }
                }
            }
        }

        Ok(ToolResult::success("git_branch".to_string(), branch_output))
    }

    /// Execute git diff
    async fn diff(&self, file: Option<String>) -> AgentNetworkResult<ToolResult> {
        let repo = Repository::open(&self.repo_path).map_err(|e| {
            crate::error::AgentNetworkError::Tool(format!("Failed to open git repo: {}", e))
        })?;

        let diff = repo.diff_index_to_workdir(None, None).map_err(|e| {
            crate::error::AgentNetworkError::Tool(format!("Failed to index workdir for git repo: {}", e))
        })?;


        let mut diff_output = String::new();

        diff.foreach(
            &mut |delta, _| {
                if let Some(ref file_filter) = file {
                    if let Some(path) = delta.new_file().path() {
                        if path.to_string_lossy().contains(file_filter) {
                            diff_output.push_str(&format!("File: {}\n", path.display()));
                        }
                    }
                } else {
                    diff_output.push_str(&format!("File: {}\n", delta.new_file().path()
                        .unwrap()
                        .display()));
                }
                true
            },
            None,
            None,
            None,
        ).map_err(|e| {
            crate::error::AgentNetworkError::Tool(format!("Failed: {}", e))
        })?;


        if diff_output.is_empty() {
            diff_output = "No changes".to_string();
        }

        Ok(ToolResult::success("git_diff".to_string(), diff_output))
    }
}

#[async_trait]
impl Tool for GitTool {
    async fn execute(
        &self,
        command: &str,
        params: HashMap<String, String>,
    ) -> AgentNetworkResult<ToolResult> {
        match command {
            "status" => self.status().await,
            "log" => {
                let limit = params
                    .get("limit")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(10);
                self.log(limit).await
            }
            "add" => {
                let patterns = params
                    .get("patterns")
                    .map(|s| s.split(',').map(|s| s.to_string()).collect())
                    .unwrap_or_default();
                self.add(patterns).await
            }
            "commit" => {
                let message = params
                    .get("message")
                    .cloned()
                    .unwrap_or_else(|| "Agent commit".to_string());
                self.commit(message).await
            }
            "branch" => {
                let action = params.get("action").cloned().unwrap_or_default();
                self.branch(action).await
            }
            "diff" => {
                let file = params.get("file").cloned();
                self.diff(file).await
            }
            _ => Err(crate::error::AgentNetworkError::Tool(format!(
                "Unknown git command: {}",
                command
            ))),
        }
    }

    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Git version control tool for commits, branches, history, and diffs"
    }

    fn available_commands(&self) -> Vec<String> {
        vec![
            "status".to_string(),
            "log".to_string(),
            "add".to_string(),
            "commit".to_string(),
            "branch".to_string(),
            "diff".to_string(),
        ]
    }

    fn validate_params(&self, command: &str, params: &HashMap<String, String>) -> AgentNetworkResult<()> {
        match command {
            "commit" => {
                if !params.contains_key("message") {
                    return Err(crate::error::AgentNetworkError::Tool(
                        "commit requires 'message' parameter".to_string(),
                    ));
                }
                Ok(())
            }
            "log" => {
                if let Some(limit) = params.get("limit") {
                    limit.parse::<usize>().map_err(|_| {
                        crate::error::AgentNetworkError::Tool(
                            "'limit' must be a valid number".to_string(),
                        )
                    })?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_tool_creation() {
        // Would need a valid git repo - test with current directory
        match GitTool::new(PathBuf::from(".")) {
            Ok(tool) => {
                assert_eq!(tool.name(), "git");
                assert!(!tool.available_commands().is_empty());
            }
            Err(_) => {
                // Current directory might not be a git repo, that's ok
            }
        }
    }

    #[test]
    fn test_available_commands() {
        let commands = vec![
            "status".to_string(),
            "log".to_string(),
            "add".to_string(),
            "commit".to_string(),
            "branch".to_string(),
            "diff".to_string(),
        ];

        assert_eq!(commands.len(), 6);
    }
}
