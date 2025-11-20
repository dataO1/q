//! Base agent trait and context types
//!
//! Defines the core Agent trait that all specialized agents implement,
//! along with context types for passing information to agents.

use ai_agent_common::{AgentType, ConversationId, ProjectScope};
use async_trait::async_trait;
use derive_more::Display;
use ollama_rs::{generation::{chat::{request::ChatMessageRequest, ChatMessage, MessageRole}, parameters::{FormatType, JsonStructure}}, Ollama};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, instrument};
use std::collections::HashMap;
use schemars::JsonSchema;
use anyhow::{Context, Result};

use crate::{agents::AgentResult, error::AgentNetworkResult};

/// Marker trait for all structured agent outputs
pub trait StructuredOutput:
    Serialize + for<'de> Deserialize<'de> + JsonSchema + Send + Sync
{
}

impl<T> StructuredOutput for T where
    T: Serialize + for<'de> Deserialize<'de> + JsonSchema + Send + Sync
{
}

// Keep your original trait for internal use with associated types
#[async_trait]
pub trait TypedAgent: Send + Sync {
    type Output: StructuredOutput;

    fn id(&self) -> &str;
    fn agent_type(&self) -> AgentType;
    fn system_prompt(&self) -> &str;
    fn model(&self) -> &str;
    fn temperature(&self) -> f32;
    fn client(&self) -> &Ollama;
    fn build_prompt(&self, context: &AgentContext) -> String;

    // async fn execute_typed(&self, context: AgentContext) -> Result<Self::Output>{}
}

// Create a dyn-compatible trait without associated types
#[async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> &str;
    fn agent_type(&self) -> AgentType;
    fn system_prompt(&self) -> &str;
    fn model(&self) -> &str;
    fn temperature(&self) -> f32;
    fn client(&self) -> &Ollama;
    fn build_prompt(&self, context: &AgentContext) -> String;

    // Return the JSON schema dynamically
    fn output_schema(&self) -> JsonStructure;

    // Execute with type-erased result
    async fn execute(&self, context: AgentContext) -> Result<AgentResult>;
}

// Blanket implementation: any TypedAgent automatically becomes an Agent
#[async_trait]
impl<T: TypedAgent> Agent for T {
    fn id(&self) -> &str { TypedAgent::id(self) }
    fn agent_type(&self) -> AgentType { TypedAgent::agent_type(self) }
    fn system_prompt(&self) -> &str { TypedAgent::system_prompt(self) }
    fn model(&self) -> &str { TypedAgent::model(self) }
    fn temperature(&self) -> f32 { TypedAgent::temperature(self) }
    fn client(&self) -> &Ollama { TypedAgent::client(self) }
    fn build_prompt(&self, context: &AgentContext) -> String {
        TypedAgent::build_prompt(self, context)
    }

    fn output_schema(&self) -> JsonStructure {
        JsonStructure::new::<T::Output>()
    }

    #[instrument(skip(self, context), fields(agent_id = %self.id()))]
    async fn execute(&self, context: AgentContext) -> Result<AgentResult> {
        let prompt_text = self.build_prompt(&context);
        let client = self.client();

        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: self.system_prompt().to_string(),
                tool_calls: vec![],
                images: None,
                thinking: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: prompt_text,
                tool_calls: vec![],
                images: None,
                thinking: None,
            },
        ];

        let json_structure = self.output_schema();
        let request = ChatMessageRequest::new(self.model().to_string(), messages)
            .format(FormatType::StructuredJson(Box::new(json_structure)));

        let response = client.send_chat_messages(request).await?;
        AgentResult::from_response(self.id(),response)
            .context("Failed to create agent result")
    }
}

/// Context passed to agents during execution
#[derive(Debug, Clone, Display)]
#[display("AgentID: {}, AgentType: {}, TaskID: {}, Description:{}, dependencies: {:?}, dependency_outputs: {:?}, rag_context: {:?}, history_context: {:?}, file_context: {:?}, tool_results: {:?}, conversation_history: {:?}, project_scope: {}, conversation_id: {}, metadata: {:?}",agent_id, agent_type, task_id, description,dependencies, dependency_outputs, rag_context, history_context, file_context, tool_results, conversation_history, project_scope, conversation_id, metadata)]
pub struct AgentContext {
    // === Agent Identification ===
    /// Which agent is executing this task
    pub agent_id: String,

    /// Type of agent (coding, planning, writing, evaluator)
    pub agent_type: AgentType,

    // === Task Information ===
    /// Unique task identifier
    pub task_id: String,

    /// Description of what the agent should do
    pub description: String,

    // === Workflow Dependencies ===
    /// IDs of tasks this task depends on
    pub dependencies: Vec<String>,

    /// Structured outputs from dependency tasks
    pub dependency_outputs: HashMap<String, Value>,

    // === Context & Retrieval ===
    /// RAG context from vector store (top-k documents)
    pub rag_context: Option<String>,

    /// Historical context from conversation history
    pub history_context: Option<String>,

    /// Relevant file paths for this task
    pub file_context: Vec<String>,

    // === Tool & Execution Results ===
    /// Results from tool executions (e.g., file reads, git ops)
    pub tool_results: Vec<ToolResult>,

    // === Conversation & Scope ===
    /// Conversation history for multi-turn interactions
    pub conversation_history: Vec<ConversationMessage>,

    /// Project scope boundaries
    pub project_scope: ProjectScope,

    /// Conversation identifier
    pub conversation_id: ConversationId,

    // === Metadata ===
    /// Additional metadata passed to agent
    pub metadata: HashMap<String, Value>,
}

impl AgentContext {
    /// Create a new agent context with required fields
    pub fn new(
        agent_id: String,
        agent_type: AgentType,
        task_id: String,
        description: String,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
    ) -> Self {
        Self {
            agent_id,
            agent_type,
            task_id,
            description,
            dependencies: vec![],
            dependency_outputs: HashMap::new(),
            rag_context: None,
            history_context: None,
            file_context: vec![],
            tool_results: vec![],
            conversation_history: vec![],
            project_scope,
            conversation_id,
            metadata: HashMap::new(),
        }
    }

    /// Add dependencies
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Add dependency outputs
    pub fn with_dependency_outputs(mut self, outputs: HashMap<String, Value>) -> Self {
        self.dependency_outputs = outputs;
        self
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

    /// Add file context
    pub fn with_file_context(mut self, files: Vec<String>) -> Self {
        self.file_context = files;
        self
    }

    /// Add tool result
    pub fn with_tool_result(mut self, result: ToolResult) -> Self {
        self.tool_results.push(result);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: Value) -> Self {
        self.metadata.insert(key.to_string(), value);
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

        // Dependency outputs first (most important for chained tasks)
        if !self.dependency_outputs.is_empty() {
            context.push_str("## Previous Task Outputs\n");
            for (task_id, output) in &self.dependency_outputs {
                context.push_str(&format!("### {}\n", task_id));
                context.push_str(&format!("{}\n\n", output));
            }
        }

        if let Some(rag) = &self.rag_context {
            context.push_str("## Retrieved Context\n");
            context.push_str(rag);
            context.push_str("\n\n");
        }

        if !self.file_context.is_empty() {
            context.push_str("## Relevant Files\n");
            for file in &self.file_context {
                context.push_str(&format!("- {}\n", file));
            }
            context.push_str("\n");
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
        let context_str = self.build_context_string();
        let dep_tokens: usize = self.dependency_outputs.values()
            .map(|v| v.to_string().len() / 4)
            .sum();

        (context_str.len() / 4) + (self.description.len() / 4) + dep_tokens + 100
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


// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn test_agent_context_builder() {
//         let ctx = AgentContext::new("task-1".to_string(), "test task".to_string())
//             .with_rag_context("some context".to_string())
//             .with_metadata("key".to_string(), "value".to_string());
//
//         assert_eq!(ctx.task_id, "task-1");
//         assert!(ctx.rag_context.is_some());
//         assert!(ctx.metadata.contains_key("key"));
//     }
//
//     #[test]
//     fn test_agent_result_builder() {
//         let result = AgentResult::from_response("agent-1", serde_json::from_str("output").unwrap()).unwrap()
//             .with_confidence(0.95)
//             .with_tokens(500)
//             .requiring_hitl();
//
//         assert_eq!(result.confidence, 0.95);
//         assert_eq!(result.tokens_used, Some(500));
//         assert!(result.requires_hitl);
//     }
//
//     #[test]
//     fn test_agent_type_display() {
//         assert_eq!(AgentType::Coding.to_string(), "Coding");
//         assert_eq!(AgentType::Planning.to_string(), "Planning");
//     }
//
//     #[test]
//     fn test_context_token_estimation() {
//         let ctx = AgentContext::new("t1".to_string(), "test".to_string());
//         let tokens = ctx.estimate_tokens();
//         assert!(tokens > 0);
//     }
// }
