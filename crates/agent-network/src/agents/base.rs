//! Base agent trait and context types
//!
//! Defines the core Agent trait that all specialized agents implement,
//! along with context types for passing information to agents.

use ai_agent_common::{AgentType, ConversationId, ProjectScope};
use async_trait::async_trait;
use derive_more::Display;
use ollama_rs::{generation::{chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse, MessageRole}, parameters::{FormatType, JsonStructure}, tools::{Tool, ToolInfo}}, Ollama};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument};
use std::{collections::HashMap, sync::Arc};
use schemars::JsonSchema;
use anyhow::{Context, Result, anyhow};

use crate::{agents::AgentResult, error::AgentNetworkResult, tools::{ToolExecution, ToolRegistry}};

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
    fn build_initial_messages(&self, context: &AgentContext) -> Vec<ChatMessage>;


    // Return the JSON schema dynamically
    fn output_schema(&self) -> JsonStructure;

    // Execute with type-erased result
    async fn execute(&self, context: AgentContext, tool_registry: Arc<Mutex<ToolRegistry>>) -> Result<AgentResult>;

    async fn execute_react_loop(
        &self,
        context: AgentContext,
        tool_registry: Arc<Mutex<ToolRegistry>>,
    ) -> Result<AgentResult>;


    async fn execute_single_shot(&self, context: AgentContext) -> Result<AgentResult> ;
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

    fn output_schema(&self) -> JsonStructure {
        JsonStructure::new::<T::Output>()
    }

    fn build_initial_messages(&self, context: &AgentContext) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // 1. Add system message (agent role/instructions)
        messages.push(ChatMessage::system(format!("#INSTRUCTIONS:\n{}",
                    self.system_prompt().to_string()
        )));

        // 2. Optionally add RAG context as additional user or system message
        if let Some(rag_context) = &context.rag_context {
            messages.push(ChatMessage::user(format!(
                "# RAG CONTEXT:\n{}",
                rag_context
            )));
        }

        // 3. Optionally add History context as additional user or system message
        if let Some(history_context) = &context.history_context {
            messages.push(ChatMessage::user(format!(
                "# HISTORY CONTEXT:\n{}",
                history_context
            )));
        }

        // 4. Optionally add Tools context as additional user or system message
        if let Ok(available_tools)= serde_json::to_string_pretty(&context.available_tools) {
            messages.push(ChatMessage::user(format!(
                "# AVAILABLE TOOLS:\n{}",
                available_tools
            )));
        }

        //TODO: add dependency_outputs here.

        // 5. Add user message (the actual task/query)
        messages.push(ChatMessage::user(format!("# USER PROMPT:\n{}",context.description)));



        messages
    }

    #[instrument(skip(self, context), fields(agent_id = %self.id()))]
    async fn execute(&self, context: AgentContext, tool_registry: Arc<Mutex<ToolRegistry>>) -> Result<AgentResult> {
        // Orchestrator provides tools for this specific execution
        let tools = tool_registry.lock().await.get_tools_info(); // From context, not agent config

        if tools.is_empty() {
            // No tools → Single-shot execution
            return self.execute_single_shot(context).await;
        }

        // Tools present → ReAct loop
        self.execute_react_loop(context, tool_registry).await
    }

    #[instrument(skip(self, context), fields(agent_id = %self.id()))]
    async fn execute_react_loop(
        &self,
        context: AgentContext,
        tool_registry: Arc<Mutex<ToolRegistry>>,
    ) -> Result<AgentResult> {
        // Initialize conversation messages with system/user context
        let mut messages = self.build_initial_messages(&context);
        let mut tool_executions: Vec<ToolExecution> = vec![];

        // Get tools info for prompt injection
        let tools_info = tool_registry.lock().await.get_tools_info();

        // Maximum allowed iterations of tool usage to avoid infinite loops
        let max_iterations = 10;

        let mut latest_response = None;

        for _iteration in 0..max_iterations {
            // Build request including tools
            let json_structure = self.output_schema();
            let request = ChatMessageRequest::new(
                self.model().to_string(),
                messages.clone(),
            )
                .format(FormatType::StructuredJson(Box::new(json_structure)))
                .tools(tools_info.clone());

            // Call Ollama chat completion with current messages and tool metadata
            let response = self.client().send_chat_messages(request).await?;

            // Add LLM assistant message to conversation history
            messages.push(response.message.clone());

            // Check if LLM requested any tool calls
            // Execute each tool call sequentially
            for tool_call in &response.message.tool_calls {
                // Parse JSON arguments
                let args: serde_json::Value = tool_call.function.arguments.clone();

                // Execute tool via the registry
                let result = tool_registry.lock().await
                    .execute(&tool_call.function.name, args.clone())
                    .await;
                    // .map_err(|e| anyhow!("Tool '{}' execution failed: {}", &tool_call.function.name, e))?;

                let tool_execution = ToolExecution::new(&tool_call.function.name, &args).with_result(&result);
                tool_executions.push(tool_execution);
                // Feed back tool result as a special Tool message in chat history
                messages.push(ChatMessage::tool(result?));
            }
            latest_response = Some(response);
        };
        match latest_response {
            Some(response) => {
                Ok(AgentResult::from_response(self.id(),response).context("Failed to create agent result")?.with_tools_exectutions(tool_executions))
            },
            None => {
                // Exceeded max iterations, likely infinite loop or error
                Err(anyhow!("Max tool call iterations reached in ReAct loop"))
            }
        }
    }

    // #[instrument(skip(self, context), fields(agent_id = %self.id()))]
    // async fn execute_single_shot(&self, context: AgentContext) -> Result<AgentResult> {
    //     // Simple LLM call without tools
    //     let response = self.call_llm_simple(&context).await?;
    //     self.extract_result(response)
    // }

    #[instrument(skip(self, context), fields(agent_id = %self.id()))]
    async fn execute_single_shot(&self, context: AgentContext) -> Result<AgentResult> {
        let messages = self.build_initial_messages(&context);

        let json_structure = self.output_schema();
        let request = ChatMessageRequest::new(self.model().to_string(), messages)
            .format(FormatType::StructuredJson(Box::new(json_structure)));

        let response = self.client().send_chat_messages(request).await?;
        AgentResult::from_response(self.id(),response)
            .context("Failed to create agent result")
    }
}

/// Context passed to agents during execution
#[derive(Debug, Display)]
#[display("AgentID: {}, AgentType: {}, TaskID: {}, Description:{}, dependencies: {:?}, dependency_outputs: {:?}, rag_context: {:?}, history_context: {:?}, file_context: {:?}, conversation_history: {:?}, project_scope: {}, conversation_id: {}, metadata: {:?}",agent_id, agent_type, task_id, description,dependencies, dependency_outputs, rag_context, history_context, file_context, conversation_history, project_scope, conversation_id, metadata)]
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
    pub available_tools: Vec<ToolInfo>,

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
            conversation_history: vec![],
            project_scope,
            conversation_id,
            metadata: HashMap::new(),
            available_tools: vec![],
        }
    }

    /// Add dependencies
    pub fn with_tools(mut self, tools: Vec<ToolInfo>) -> Self {
        self.available_tools = tools;
        self
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
