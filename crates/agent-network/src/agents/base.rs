//! Base agent trait and common types

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AgentNetworkResult;

#[async_trait]
pub trait Agent: Send + Sync {
    /// Execute agent task with given context
    async fn execute(&self, context: AgentContext) -> AgentNetworkResult<super::AgentResponse>;

    /// Get agent ID
    fn id(&self) -> &str;

    /// Get agent type
    fn agent_type(&self) -> AgentType;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AgentType {
    Coding,
    Planning,
    Writing,
    Evaluator,
}

#[derive(Debug, Clone)]
pub struct AgentContext {
    pub task_id: String,
    pub description: String,
    pub rag_context: Option<String>,
    pub history_context: Option<String>,
    pub tool_results: Vec<ToolResult>,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_name: String,
    pub output: String,
}
