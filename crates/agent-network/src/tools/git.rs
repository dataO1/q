// use async_trait::async_trait;
// use anyhow::{anyhow, Result};
// use git2::{Repository, Signature};
// use serde_json::{Value, json};
// use std::path::PathBuf;
// use tokio::task;
// use tracing::{debug, info};
//
// #[derive(Debug)]
// pub struct GitTool {
//     repo_path: PathBuf,
// }
//
// impl GitTool {
//     pub fn new(repo_path: PathBuf) -> Result<Self> {
//         // Just check repo existence synchronously here
//         Repository::open(&repo_path).map_err(|e| anyhow!("Failed to open git repo: {}", e))?;
//         info!("GitTool initialized for: {}", repo_path.display());
//         Ok(Self { repo_path })
//     }
//
//     // Helper to do blocking git operations on a dedicated thread pool
//     async fn open_repo(&self) -> Result<Repository> {
//         let path = self.repo_path.clone();
//         task::spawn_blocking(move || Repository::open(path))
//             .await?
//             .map_err(|e| anyhow!("Failed to open git repo: {}", e))
//     }
//
//     // Example: status command
//     async fn status(&self) -> Result<String> {
//         let repo_path = self.repo_path.clone();
//         let statuses = tokio::task::spawn_blocking(move || -> Result<git2::Statuses, anyhow::Error> {
//             let repo = Repository::open(repo_path)?;
//             Ok(repo.statuses(None)?)
//         })
//         .await??;
//
//         let mut output = String::new();
//         for entry in statuses.iter() {
//             let path = entry.path().unwrap_or("unknown");
//             let status_str = if entry.status().is_wt_new() {
//                 "new"
//             } else if entry.status().is_wt_modified() {
//                 "modified"
//             } else if entry.status().is_wt_deleted() {
//                 "deleted"
//             } else if entry.status().is_index_modified() {
//                 "staged-modified"
//             } else if entry.status().is_index_new() {
//                 "staged-new"
//             } else {
//                 "unknown"
//             };
//             output.push_str(&format!("{}: {}\n", path, status_str));
//         }
//         Ok(output)
//     }
//
//     // Similarly implement async `log`, `add`, `commit`, `branch`, `diff` here,
//     // running all blocking operations inside `task::spawn_blocking`.
//
//     // To save space, omitted here - implement following pattern above.
// }
//
// #[async_trait]
// impl crate::tools::ToolExecutor for GitTool {
//     fn name(&self) -> &'static str {
//         "git"
//     }
//
//     fn description(&self) -> &'static str {
//         "Git version control tool for commits, branches, history, and diffs"
//     }
//
//     fn provide_tool_info(&self) -> ollama_rs::generation::tools::ToolInfo {
//         // Simple JSON schema for command and parameters
//         let parameters = json!({
//             "type": "object",
//             "properties": {
//                 "command": {
//                     "type": "string",
//                     "enum": ["status", "log", "add", "commit", "branch", "diff"]
//                 },
//                 "message": {
//                     "type": "string"
//                 },
//                 "patterns": {
//                     "type": "string",
//                     "description": "Comma separated list of file patterns"
//                 },
//                 "limit": {
//                     "type": "integer"
//                 },
//                 "file": {
//                     "type": "string"
//                 },
//                 "action": {
//                     "type": "string"
//                 }
//             },
//             "required": ["command"]
//         });
//
//         ollama_rs::generation::tools::ToolInfo {
//             tool_type: ollama_rs::generation::tools::ToolType::Function,
//             function: ollama_rs::generation::tools::ToolFunctionInfo {
//                 name: self.name().to_string(),
//                 description: self.description().to_string(),
//                 parameters: serde_json::from_value(parameters).unwrap(),
//             },
//         }
//     }
//
//     async fn call(&mut self, parameters: Value) -> Result<String> {
//         let command = parameters.get("command")
//             .and_then(Value::as_str)
//             .ok_or_else(|| anyhow!("Missing 'command' field"))?;
//
//         match command {
//             "status" => self.status().await,
//
//             // For "log", parse limit param:
//             "log" => {
//                 let limit = parameters.get("limit")
//                     .and_then(Value::as_u64)
//                     .unwrap_or(10) as usize;
//                 // Implement log following same spawn_blocking pattern as status
//                 Err(anyhow!("log command not yet implemented"))
//             }
//
//             // For "add", parse patterns:
//             "add" => {
//                 let patterns = parameters.get("patterns")
//                     .and_then(Value::as_str)
//                     .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())
//                     .unwrap_or_default();
//                 // Implement add similar to status
//                 Err(anyhow!("add command not yet implemented"))
//             }
//
//             // Similarly implement commit, branch, diff...
//
//             _ => Err(anyhow!("Unknown git command: {}", command)),
//         }
//     }
// }
