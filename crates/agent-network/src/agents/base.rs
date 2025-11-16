//! Base agent trait and context types
//!
//! Defines the core Agent trait that all specialized agents implement,
//! along with context types for passing information to agents.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{agents::AgentResult, error::AgentNetworkResult};

/// Core Agent trait - all agents must implement this
#[async_trait]
pub trait Agent: Send + Sync {
    /// Execute agent task with given context
    async fn execute(&self, context: AgentContext) -> AgentNetworkResult<AgentResult>;

    /// Get agent identifier
    fn id(&self) -> &str;

    /// Get agent type
    fn agent_type(&self) -> AgentType;

    /// Get agent description
    fn description(&self) -> &str;
}

/// Agent type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    Coding,
    Planning,
    Writing,
    Evaluator,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Coding => write!(f, "Coding"),
            Self::Planning => write!(f, "Planning"),
            Self::Writing => write!(f, "Writing"),
            Self::Evaluator => write!(f, "Evaluator"),
        }
    }
}

/// Context passed to agents during execution
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Unique task identifier
    pub task_id: String,

    /// Description of what the agent should do
    pub description: String,

    /// RAG context from vector store (top-k documents)
    pub rag_context: Option<String>,

    /// Historical context from conversation history
    pub history_context: Option<String>,

    /// Results from tool executions (e.g., file reads, git ops)
    pub tool_results: Vec<ToolResult>,

    /// Additional metadata passed to agent
    pub metadata: HashMap<String, String>,

    /// Conversation history for multi-turn interactions
    pub conversation_history: Vec<ConversationMessage>,
}

impl AgentContext {
    /// Create a new agent context
    pub fn new(task_id: String, description: String) -> Self {
        Self {
            task_id,
            description,
            rag_context: None,
            history_context: None,
            tool_results: vec![],
            metadata: HashMap::new(),
            conversation_history: vec![],
        }
    }

    /// Add RAG context
    pub fn with_rag_context(mut self, context: String) -> Self {
        self.rag_context = Some(context);
        self
    }

    /// Add history context
    pub fn with_history_context(mut self, context: String) -> Self {
        self.history_context = Some(context);
        self
    }

    /// Add tool result
    pub fn with_tool_result(mut self, result: ToolResult) -> Self {
        self.tool_results.push(result);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Add conversation message
    pub fn with_message(mut self, message: ConversationMessage) -> Self {
        self.conversation_history.push(message);
        self
    }

    /// Build system prompt from context
    pub fn build_context_string(&self) -> String {
        let mut context = String::new();

        if let Some(rag) = &self.rag_context {
            context.push_str("## Retrieved Context\n");
            context.push_str(rag);
            context.push_str("\n\n");
        }

        if let Some(history) = &self.history_context {
            context.push_str("## Recent History\n");
            context.push_str(history);
            context.push_str("\n\n");
        }

        for tool_result in &self.tool_results {
            context.push_str(&format!("## Tool: {}\n", tool_result.tool_name));
            context.push_str(&tool_result.output);
            context.push_str("\n\n");
        }

        context
    }

    /// Get estimated token count for context
    pub fn estimate_tokens(&self) -> usize {
        // Rough estimate: 1 token ≈ 4 characters
        let context_str = self.build_context_string();
        (context_str.len() / 4) + (self.description.len() / 4) + 100
    }
}

/// Result from tool execution
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Name of the tool executed
    pub tool_name: String,

    /// Output from tool execution
    pub output: String,

    /// Whether tool execution succeeded
    pub success: bool,

    /// Error message if failed
    pub error: Option<String>,
}

impl ToolResult {
    /// Create successful tool result
    pub fn success(tool_name: String, output: String) -> Self {
        Self {
            tool_name,
            output,
            success: true,
            error: None,
        }
    }

    /// Create failed tool result
    pub fn error(tool_name: String, error: String) -> Self {
        Self {
            tool_name,
            output: String::new(),
            success: false,
            error: Some(error),
        }
    }
}

/// Conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversationMessage {
    /// User message
    User(String),

    /// Assistant response
    Assistant(String),

    /// System message
    System(String),
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_context_builder() {
        let ctx = AgentContext::new("task-1".to_string(), "test task".to_string())
            .with_rag_context("some context".to_string())
            .with_metadata("key".to_string(), "value".to_string());

        assert_eq!(ctx.task_id, "task-1");
        assert!(ctx.rag_context.is_some());
        assert!(ctx.metadata.contains_key("key"));
    }

    #[test]
    fn test_agent_result_builder() {
        let result = AgentResult::new("agent-1".to_string(), "output".to_string())
            .with_confidence(0.95)
            .with_tokens(500)
            .requiring_hitl();

        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.tokens_used, Some(500));
        assert!(result.requires_hitl);
    }

    #[test]
    fn test_agent_type_display() {
        assert_eq!(AgentType::Coding.to_string(), "Coding");
        assert_eq!(AgentType::Planning.to_string(), "Planning");
    }

    #[test]
    fn test_context_token_estimation() {
        let ctx = AgentContext::new("t1".to_string(), "test".to_string());
        let tokens = ctx.estimate_tokens();
        assert!(tokens > 0);
    }
}
