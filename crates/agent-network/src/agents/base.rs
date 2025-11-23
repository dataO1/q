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
use tracing::{debug, info, error, instrument};
use std::{collections::HashMap, sync::Arc};
use schemars::JsonSchema;
use anyhow::{Context, Result, anyhow};

use crate::{agents::AgentResult, error::AgentNetworkResult, tools::{ToolExecution, ToolRegistry}};

/// ReAct step output for semantic stop conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactStepOutput {
    pub status: String,
    pub reasoning: String,
    pub result: Option<Value>,
}

/// Workflow step execution mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepExecutionMode {
    /// Single LLM call without tools - fast for analysis/planning
    OneShot,
    /// ReAct loop with tools - for tasks requiring tool usage
    ReAct { max_iterations: Option<usize> },
}

/// Reusable workflow step definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub description: String,
    pub execution_mode: StepExecutionMode,
    pub required_tools: Vec<String>,
    pub parameters: HashMap<String, Value>, // Step-specific configuration
}

/// Result of executing a workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub tool_executions: Vec<ToolExecution>,
}

/// Workflow execution state passed between steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub step_results: Vec<StepResult>, // Sequential results
    pub shared_context: HashMap<String, Value>, // Data shared between steps
}

impl WorkflowState {
    pub fn new() -> Self {
        Self {
            step_results: Vec::new(),
            shared_context: HashMap::new(),
        }
    }

    pub fn latest_result(&self) -> Option<&StepResult> {
        self.step_results.last()
    }

    pub fn get_step_result(&self, step_id: &str) -> Option<&StepResult> {
        self.step_results.iter().find(|r| r.step_id == step_id)
    }

    pub fn add_step_result(&mut self, result: StepResult) {
        self.step_results.push(result);
    }
}

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

    /// Define the workflow steps for this agent
    /// Each agent can define its own sequence of steps, each either OneShot or ReAct
    fn define_workflow_steps(&self, context: &AgentContext) -> Vec<WorkflowStep>;

    /// Execute workflow steps sequentially
    async fn execute_workflow(
        &self,
        context: AgentContext,
        workflow_steps: Vec<WorkflowStep>,
        tool_registry: Arc<ToolRegistry>
    ) -> Result<AgentResult> {
        let mut workflow_state = WorkflowState::new();
        let mut final_result = None;
        let mut all_tool_executions = Vec::new();

        for (step_index, step) in workflow_steps.iter().enumerate() {
            debug!("Executing workflow step {}/{}: {}", step_index + 1, workflow_steps.len(), step.name);

            // Update context with workflow state for this step
            let mut updated_context = context.clone();
            updated_context.metadata.insert("workflow_step_id".to_string(), serde_json::to_value(&step.id)?);
            updated_context.metadata.insert("workflow_state".to_string(), serde_json::to_value(&workflow_state)?);

            // Execute the individual step
            let step_result = match &step.execution_mode {
                StepExecutionMode::OneShot => {
                    self.execute_step_oneshot(&updated_context, step).await
                }
                StepExecutionMode::ReAct { max_iterations } => {
                    self.execute_step_react(&updated_context, step, tool_registry.clone(), *max_iterations).await
                }
            };

            match step_result {
                Ok(result) => {
                    // Collect tool executions
                    all_tool_executions.extend(result.tool_executions.clone());

                    // Update shared context with step results
                    if let Some(output) = &result.output {
                        workflow_state.shared_context.insert(step.id.clone(), output.clone());
                    }

                    workflow_state.add_step_result(result.clone());
                    final_result = Some(result.output.unwrap_or_default());

                    debug!("Step '{}' completed successfully", step.name);
                }
                Err(e) => {
                    let error_msg = format!("Workflow step '{}' failed: {}", step.name, e);
                    error!("{}", error_msg);

                    let failed_result = StepResult {
                        step_id: step.id.clone(),
                        success: false,
                        output: None,
                        error: Some(error_msg.clone()),
                        tool_executions: Vec::new(),
                    };
                    workflow_state.add_step_result(failed_result);

                    return Err(anyhow!(error_msg));
                }
            }
        }

        // Create final agent result combining all workflow steps
        Ok(AgentResult {
            agent_id: self.id().to_string(),
            output: final_result.unwrap_or_default(),
            confidence: 0.8, // Workflow completion confidence
            requires_hitl: false,
            tokens_used: None, // Could aggregate from steps
            reasoning: Some(format!("Completed {}-step workflow: {}",
                workflow_steps.len(),
                workflow_steps.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(" → ")
            )),
            tool_executions: all_tool_executions,
        })
    }

    /// Execute a single OneShot workflow step
    async fn execute_step_oneshot(&self, context: &AgentContext, step: &WorkflowStep) -> Result<StepResult> {
        // Build messages for this specific step
        let mut messages = vec![
            ChatMessage::system(format!("# STEP: {}\n{}\n\n# INSTRUCTIONS:\n{}",
                step.name,
                step.description,
                self.system_prompt()
            ))
        ];

        // Add relevant context
        if let Some(rag_context) = &context.rag_context {
            messages.push(ChatMessage::user(format!("# RAG CONTEXT:\n{}", rag_context)));
        }

        if !context.dependency_outputs.is_empty() {
            let dependency_msg = serde_json::to_string_pretty(&context.dependency_outputs)
                .unwrap_or_else(|_| "Failed to serialize dependency outputs".to_string());
            messages.push(ChatMessage::user(format!("# PREVIOUS TASK OUTPUTS:\n{}", dependency_msg)));
        }

        // Add workflow state if available
        if let Some(workflow_state) = context.metadata.get("workflow_state") {
            messages.push(ChatMessage::user(format!("# WORKFLOW CONTEXT:\n{}",
                serde_json::to_string_pretty(workflow_state).unwrap_or_default())));
        }

        // Add step parameters
        if !step.parameters.is_empty() {
            messages.push(ChatMessage::user(format!("# STEP PARAMETERS:\n{}",
                serde_json::to_string_pretty(&step.parameters).unwrap_or_default())));
        }

        // Add main user prompt
        messages.push(ChatMessage::user(format!("# USER PROMPT:\n{}", context.description)));

        // Execute LLM call
        let json_structure = JsonStructure::new::<Self::Output>();
        let request = ChatMessageRequest::new(self.model().to_string(), messages)
            .format(FormatType::StructuredJson(Box::new(json_structure)));

        let response = self.client().send_chat_messages(request).await?;

        // Parse response
        let parsed_output = response.message.content
            .strip_prefix("```json")
            .unwrap_or(&response.message.content)
            .strip_suffix("```")
            .unwrap_or(&response.message.content);

        let output = serde_json::from_str::<Value>(parsed_output)?;

        Ok(StepResult {
            step_id: step.id.clone(),
            success: true,
            output: Some(output),
            error: None,
            tool_executions: vec![], // OneShot doesn't use tools
        })
    }

    /// Execute a single ReAct workflow step
    async fn execute_step_react(
        &self,
        context: &AgentContext,
        step: &WorkflowStep,
        tool_registry: Arc<ToolRegistry>,
        max_iterations: Option<usize>
    ) -> Result<StepResult> {
        // Build initial messages for this step
        let mut messages = vec![
            ChatMessage::system(format!("# STEP: {}\n{}\n\n# INSTRUCTIONS:\n{}",
                step.name,
                step.description,
                self.system_prompt()
            ))
        ];

        // Add context (similar to oneshot but will be extended in ReAct loop)
        if let Some(rag_context) = &context.rag_context {
            messages.push(ChatMessage::user(format!("# RAG CONTEXT:\n{}", rag_context)));
        }

        if !context.dependency_outputs.is_empty() {
            let dependency_msg = serde_json::to_string_pretty(&context.dependency_outputs)
                .unwrap_or_else(|_| "Failed to serialize dependency outputs".to_string());
            messages.push(ChatMessage::user(format!("# PREVIOUS TASK OUTPUTS:\n{}", dependency_msg)));
        }

        // Add workflow state
        if let Some(workflow_state) = context.metadata.get("workflow_state") {
            messages.push(ChatMessage::user(format!("# WORKFLOW CONTEXT:\n{}",
                serde_json::to_string_pretty(workflow_state).unwrap_or_default())));
        }

        // Add step parameters
        if !step.parameters.is_empty() {
            messages.push(ChatMessage::user(format!("# STEP PARAMETERS:\n{}",
                serde_json::to_string_pretty(&step.parameters).unwrap_or_default())));
        }

        // Get tools info for function calling schema only (not for message injection)
        let tools_info = tool_registry.get_tools_info();

        // Add main user prompt
        messages.push(ChatMessage::user(format!("# USER PROMPT:\n{}", context.description)));

        // Execute ReAct loop for this step
        let mut tool_executions = Vec::new();
        let max_iter = max_iterations.unwrap_or(10);
        let mut latest_response = None;

        for _iteration in 0..max_iter {
            // Build request with tools
            let json_structure = JsonStructure::new::<Self::Output>();
            let request = ChatMessageRequest::new(self.model().to_string(), messages.clone())
                .format(FormatType::StructuredJson(Box::new(json_structure)))
                .tools(tools_info.clone());

            let response = self.client().send_chat_messages(request).await?;
            latest_response = Some(response.message.content.clone());

            // Add assistant response to message history to maintain conversation context
            // This ensures the model can build on its previous reasoning in subsequent iterations
            messages.push(ChatMessage::assistant(response.message.content.clone()));

            // Process tool calls if any
            if !response.message.tool_calls.is_empty() {
                let tool_calls = response.message.tool_calls;
                for tool_call in tool_calls {
                    let mut execution = ToolExecution::new(&tool_call.function.name, &tool_call.function.arguments);

                    // Execute tool
                    match tool_registry.execute(&tool_call.function.name, tool_call.function.arguments).await {
                        Ok(result) => {
                            execution.result = Some(result.clone());
                            messages.push(ChatMessage::user(format!("Tool Result: {}", result)));
                        }
                        Err(e) => {
                            let error_msg = e.to_string();
                            execution.error = Some(error_msg.clone());
                            messages.push(ChatMessage::user(format!("Tool Error: {}", error_msg)));
                        }
                    }

                    tool_executions.push(execution);
                }
            } else {
                // No tool calls - check for semantic completion
                // Try to parse response for semantic stop conditions
                if let Ok(step_output) = serde_json::from_str::<ReactStepOutput>(&response.message.content) {
                    if step_output.status.to_lowercase() == "done" ||
                       step_output.status.to_lowercase() == "complete" ||
                       step_output.status.to_lowercase() == "finished" {
                        debug!("ReAct step completed with semantic stop condition: {}", step_output.status);
                        break;
                    }
                }
                // If no valid semantic output but no tool calls, step is complete
                debug!("ReAct step completed - no tool calls and no explicit continuation signal");
                break;
            }
        }

        // Parse final response
        let final_content = latest_response.unwrap_or_default();

        let output = serde_json::from_str(&final_content)
            .unwrap_or_else(|_| Value::String(final_content));

        Ok(StepResult {
            step_id: step.id.clone(),
            success: true,
            output: Some(output),
            error: None,
            tool_executions,
        })
    }

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
    async fn execute(&self, context: AgentContext, tool_registry: Arc<ToolRegistry>) -> Result<AgentResult>;


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

        // // 4. Optionally add Tools context as additional user or system message
        // if let Ok(available_tools)= serde_json::to_string_pretty(&context.available_tools) {
        //     messages.push(ChatMessage::user(format!(
        //         "# AVAILABLE TOOLS:\n{}",
        //         available_tools
        //     )));
        // }

        // 4.5. Add dependency outputs from previous tasks (includes tool executions!)
        if !context.dependency_outputs.is_empty() {
            let dependency_msg = serde_json::to_string_pretty(&context.dependency_outputs)
                .unwrap_or_else(|_| "Failed to serialize dependency outputs".to_string());
            messages.push(ChatMessage::user(format!(
                "# PREVIOUS TASK OUTPUTS:\n{}",
                dependency_msg
            )));
        }

        // 5. Add user message (the actual task/query)
        messages.push(ChatMessage::user(format!("# USER PROMPT:\n{}",context.description)));



        messages
    }

    #[instrument(skip(self, context), fields(context))]
    async fn execute(&self, context: AgentContext, tool_registry: Arc<ToolRegistry>) -> Result<AgentResult> {
        // All agents use workflow execution
        let workflow_steps = self.define_workflow_steps(&context);
        self.execute_workflow(context, workflow_steps, tool_registry).await
    }

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
#[derive(Debug, Display, Clone)]
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
