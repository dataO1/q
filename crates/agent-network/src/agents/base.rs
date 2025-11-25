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
use tracing::{debug, info, error, warn, instrument, Instrument};
use std::{collections::{HashMap, HashSet}, sync::Arc};
use schemars::JsonSchema;
use anyhow::{Context, Result, anyhow};
use ollama_rs::coordinator::Coordinator;

use crate::{agents::AgentResult, tools::{filesystem::FILESYSTEM_PREAMBLE, DynamicTool, ToolExecution, ToolSet}};

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
    pub formatted: bool,
    pub parameters: HashMap<String, Value>, // Step-specific configuration
}

/// Result of executing a workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub tool_executions: Vec<ToolExecution>,
}

/// Workflow execution state passed between steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub step_results: Vec<StepResult>, // Sequential results
    pub shared_context: HashMap<String, String>, // Data shared between steps
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

    /// Estimate token count (rough: 1 token ≈ 4 characters)
    fn estimate_tokens(text: &str) -> usize {
        (text.len() / 4).max(1)
    }

    /// Execute workflow steps sequentially
    async fn execute_workflow(
        &self,
        context: AgentContext,
        workflow_steps: Vec<WorkflowStep>,
        tools: ToolSet
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
                    self.execute_step_react(&updated_context, step, tools.clone(), *max_iterations).await
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
        let final_result = serde_json::from_str(&final_result.unwrap_or_default());

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
    #[instrument(name = "agent_oneshot_step", skip(self, context), fields(
        step_id = %step.id,
        step_name = %step.name,
        agent_id = %self.id(),
        step.description = tracing::field::Empty,
        step.parameters = tracing::field::Empty,
        context.description = tracing::field::Empty,
        context.dependencies = tracing::field::Empty,
        context.dependency_outputs = tracing::field::Empty,
        rag_context.length = tracing::field::Empty,
        rag_context.content = tracing::field::Empty,
        history_context.length = tracing::field::Empty,
        history_context.content = tracing::field::Empty,
        workflow_state = tracing::field::Empty,
        llm.messages_sent = tracing::field::Empty,
        llm.messages_json = tracing::field::Empty,
        llm.response_content = tracing::field::Empty,
        result.success = tracing::field::Empty,
        result.output_length = tracing::field::Empty
    ))]
    async fn execute_step_oneshot(&self, context: &AgentContext, step: &WorkflowStep) -> Result<StepResult> {
        // Record comprehensive input details as span attributes for Jaeger visibility
        let current_span = tracing::Span::current();
        current_span.record("step.description", step.description.as_str());
        current_span.record("step.parameters", serde_json::to_string(&step.parameters).unwrap_or_default().as_str());
        current_span.record("context.description", context.description.as_str());
        current_span.record("context.dependencies", format!("{:?}", context.dependencies).as_str());
        current_span.record("context.dependency_outputs", serde_json::to_string(&context.dependency_outputs).unwrap_or_default().as_str());

        if let Some(rag_context) = &context.rag_context {
            current_span.record("rag_context.length", rag_context.len());
            current_span.record("rag_context.content", rag_context.as_str());
        }
        if let Some(history_context) = &context.history_context {
            current_span.record("history_context.length", history_context.len());
            current_span.record("history_context.content", history_context.as_str());
        }
        if let Some(workflow_state) = context.metadata.get("workflow_state") {
            current_span.record("workflow_state", serde_json::to_string(workflow_state).unwrap_or_default().as_str());
        }

        // Also log for console debugging
        debug!(target: "agent_execution", step_id = %step.id, step_name = %step.name, "OneShot step starting with context");
        debug!(target: "agent_execution", "Step description: {}", step.description);
        if let Some(rag_context) = &context.rag_context {
            debug!(target: "agent_execution", "RAG context: {} chars", rag_context.len());
        }
        if let Some(history_context) = &context.history_context {
            debug!(target: "agent_execution", "History context: {} chars", history_context.len());
        }
        let messages = self.build_initial_message(&context, &step, None);

        // Execute LLM call
        let json_structure = JsonStructure::new::<Self::Output>();
        let mut request = ChatMessageRequest::new(self.model().to_string(), messages.clone());
        if step.formatted{
            request = request.format(FormatType::StructuredJson(Box::new(json_structure)));
        }

        debug!(target: "agent_execution", "Starting LLM call for OneShot step '{}' (model: {}, messages: {}, formatted: {})",
            step.name, self.model(), messages.len(), step.formatted);

        // Execute LLM call with enhanced span instrumentation and real-time events
        let prompt_tokens = Self::estimate_tokens(&messages.iter().map(|m| m.content.clone()).collect::<Vec<_>>().join("\n"));

        // Create span with all business attributes upfront - agent info in fields
        let agent_name = format!("{}", self.agent_type());
        let llm_span = tracing::info_span!(
            "llm_inference",
            agent_name = %agent_name,
            execution_mode = "oneshot",
            step_id = %step.id,
            step_name = %step.name,
            "llm.provider" = "ollama",
            "llm.model" = %self.model(),
            "llm.token_count.prompt" = prompt_tokens,
            "llm.token_count.completion" = tracing::field::Empty,
            "llm.latency_per_token" = tracing::field::Empty,
            message_count = messages.len()
        );

        let response = async {
            // Record request start event with details
            info!(target: "llm_inference", "llm_request_started: model={}, prompt_tokens={}, message_count={}, execution_mode=oneshot",
                self.model(), prompt_tokens, messages.len());

            let start_time = std::time::Instant::now();
            info!("Starting LLM inference for OneShot step '{}' (model: {}, prompt tokens: {})",
                step.name, self.model(), prompt_tokens);

            // Execute the actual LLM call
            // let response = self.client().send_chat_messages(request).await?;
            let response = self.client().send_chat_messages(request).await?;
            let duration = start_time.elapsed();

            // Calculate completion metrics immediately
            let completion_tokens = Self::estimate_tokens(&response.message.content);
            let latency_per_token = if completion_tokens > 0 {
                duration.as_millis() / completion_tokens as u128
            } else { 0 };

            // Record response received event with details
            info!(target: "llm_inference", "llm_response_received: completion_tokens={}, total_latency_ms={}, latency_per_token_ms={}, response_length={}",
                completion_tokens, duration.as_millis(), latency_per_token, response.message.content.len());

            // Record completion metrics in span
            let current_span = tracing::Span::current();
            current_span.record("llm.token_count.completion", &completion_tokens);
            current_span.record("llm.latency_per_token", &format!("{}ms", latency_per_token));

            info!("LLM inference completed for OneShot step '{}' (response: {} chars, completion tokens: {}, latency: {}ms)",
                step.name, response.message.content.len(), completion_tokens, duration.as_millis());

            Ok::<_, anyhow::Error>(response)
        }.instrument(llm_span).await?;

        // Record LLM response and results as span attributes for Jaeger visibility
        current_span.record("llm.response_content", response.message.content.as_str());
        current_span.record("result.success", true);
        current_span.record("result.output_length", response.message.content.len());

        debug!(target: "agent_execution", "LLM call completed for OneShot step '{}' (response length: {} chars)",
            step.name, response.message.content.len());
        debug!(target: "agent_execution", "Response preview: {}",
            response.message.content.chars().take(200).collect::<String>());

        // Parse response
        // let parsed_output = response.message.content
        //     .strip_prefix("```json")
        //     .unwrap_or(&response.message.content)
        //     .strip_suffix("```")
        //     .unwrap_or(&response.message.content);
        //
        // let output = serde_json::from_str::<Value>(parsed_output)?;

        let step_result = StepResult {
            step_id: step.id.clone(),
            success: true,
            output: Some(response.message.content.clone()),
            error: None,
            tool_executions: vec![], // OneShot doesn't use tools
        };

        debug!(target: "agent_execution", "OneShot step '{}' completed successfully", step.name);

        Ok(step_result)
    }

    /// Execute a single ReAct workflow step
    #[instrument(name = "agent_react_step", skip(self, context, tools), fields(
        step_id = %step.id,
        step_name = %step.name,
        agent_id = %self.id(),
        max_iterations = ?max_iterations,
        step.description = tracing::field::Empty,
        context.description = tracing::field::Empty,
        rag_context.length = tracing::field::Empty,
        history_context.length = tracing::field::Empty,
        total_iterations = tracing::field::Empty,
        total_tool_executions = tracing::field::Empty,
        final_response_length = tracing::field::Empty
    ))]
    async fn execute_step_react(
        &self,
        context: &AgentContext,
        step: &WorkflowStep,
        mut tools: ToolSet,
        max_iterations: Option<usize>
    ) -> Result<StepResult> {
        // Record comprehensive input details as span attributes for Jaeger visibility
        let current_span = tracing::Span::current();
        current_span.record("step.description", step.description.as_str());
        current_span.record("context.description", context.description.as_str());

        if let Some(rag_context) = &context.rag_context {
            current_span.record("rag_context.length", rag_context.len());
        }
        if let Some(history_context) = &context.history_context {
            current_span.record("history_context.length", history_context.len());
        }

        debug!(target: "agent_execution", step_id = %step.id, step_name = %step.name, "ReAct step starting with Coordinator");
        debug!(target: "agent_execution", "Step description: {}", step.description);
        if let Some(rag_context) = &context.rag_context {
            debug!(target: "agent_execution", "RAG context: {} chars", rag_context.len());
        }
        if let Some(history_context) = &context.history_context {
            debug!(target: "agent_execution", "History context: {} chars", history_context.len());
        }
        let messages = self.build_initial_message(context,step, Some(&tools));

        // Dynamically add required tools for this step
        for tool_name in &step.required_tools {
            tools.ensure_filesystem_tool(tool_name);
            debug!(target: "agent_execution", "Ensured tool '{}' is available for step '{}'", tool_name, step.name);
        }

        // Create Coordinator with tools
        let mut coordinator = Coordinator::new(
            self.client().clone(),
            self.model().to_string(),
            vec![] // Start with empty history
        );
        let json_structure = JsonStructure::new::<Self::Output>();
        if step.formatted{
            coordinator = coordinator.format(FormatType::StructuredJson(Box::new(json_structure)));
        }

        // Dynamically add tools based on required_tools for this step
        for tool_name in &step.required_tools {
            if let Some(dynamic_tool) = tools.get_tool(tool_name) {
                match dynamic_tool {
                    DynamicTool::WriteFile(tool) => {
                        coordinator = coordinator.add_tool(tool.as_ref().clone());
                    }
                    DynamicTool::ReadFile(tool) => {
                        coordinator = coordinator.add_tool(tool.as_ref().clone());
                    }
                    DynamicTool::ListDirectory(tool) => {
                        coordinator = coordinator.add_tool(tool.as_ref().clone());
                    }
                    DynamicTool::CreateDirectory(tool) => {
                        coordinator = coordinator.add_tool(tool.as_ref().clone());
                    }
                    DynamicTool::FileExists(tool) => {
                        coordinator = coordinator.add_tool(tool.as_ref().clone());
                    }
                    DynamicTool::FileMetadata(tool) => {
                        coordinator = coordinator.add_tool(tool.as_ref().clone());
                    }
                    DynamicTool::DeleteFile(tool) => {
                        coordinator = coordinator.add_tool(tool.as_ref().clone());
                    }
                    DynamicTool::Lsp(tool) => {
                        coordinator = coordinator.add_tool(tool.as_ref().clone());
                    }
                }
                debug!(target: "agent_execution", "Added tool '{}' to coordinator for step '{}'", tool_name, step.name);
            } else {
                warn!(target: "agent_execution", "Required tool '{}' not found for step '{}'", tool_name, step.name);
            }
        }

        debug!(target: "agent_execution", "Starting Coordinator chat for step '{}'", step.name);
        if step.required_tools.is_empty() {
            info!(target: "agent_execution", "Using Coordinator for ReAct step '{}' with no tools", step.name);
        } else {
            info!(target: "agent_execution", "Using Coordinator for ReAct step '{}' with tools: {}", step.name, step.required_tools.join(", "));
        }
        // Execute the coordinator chat
        // let start_time = std::time::Instant::now();
        let response = coordinator
            .chat(messages).await
            .map_err(|e| anyhow::anyhow!("Coordinator chat failed: {}", e))?;
        // let duration = start_time.elapsed();
        //
        // current_span.record("final_response_length", response.len());

        // debug!(target: "agent_execution", "Coordinator completed for step '{}' (duration: {}ms, response length: {} chars)",
            // step.name, duration.as_millis(), response.len());

        // For now, we'll create a simplified tool execution tracking
        // In the future, we could extract tool calls from the coordinator's history
        let tool_executions = vec![]; // TODO: Extract from coordinator if needed

        let step_result = StepResult {
            step_id: step.id.clone(),
            success: true,
            output: Some(response.message.content),
            error: None,
            tool_executions,
        };

        debug!(target: "agent_execution", "ReAct step '{}' completed successfully using Coordinator", step.name);
        Ok(step_result)
    }

    /// Execute a single ReAct workflow step
    #[instrument(name = "agent_react_step", skip(self, context, step), fields())]
    fn build_initial_message(&self,context: &AgentContext, step: &WorkflowStep, tools: Option<&ToolSet>)-> Vec<ChatMessage>{

        let current_span = tracing::Span::current();
        // Build messages for this specific step
        let mut messages = vec![
            ChatMessage::system(format!("# STEP: {}\n{}\n\n# INSTRUCTIONS:\n{}",
                step.name,
                step.description,
                self.system_prompt()
            ))
        ];

        if !context.dependency_outputs.is_empty() {
            let mut dependency_msg = format!("# PREVIOUS TASK OUTPUTS ({} tasks completed):\n\n", context.dependency_outputs.len());

            for (task_id, output) in &context.dependency_outputs {
                // Extract attribution metadata if available
                let agent_id = output.get("agent_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown-agent");
                let task_description = output.get("task_description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No description");
                let completed_at = output.get("completed_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown time");

                dependency_msg.push_str(&format!(
                    "## Task: {} | Agent: {} | Completed: {}\n",
                    task_description.chars().take(50).collect::<String>(),
                    agent_id,
                    completed_at
                ));

                // Add the actual output content
                let output_content = serde_json::to_string_pretty(output)
                    .unwrap_or_else(|_| "Failed to serialize output".to_string());
                dependency_msg.push_str(&format!("{}\n\n", output_content));
            }

            messages.push(ChatMessage::user(dependency_msg));
        }

        // Add workflow state if available
        if let Some(workflow_state_value) = context.metadata.get("workflow_state") {
            // Try to parse the workflow state for better formatting
            if let Ok(workflow_state) = serde_json::from_value::<WorkflowState>(workflow_state_value.clone()) {
                let mut workflow_msg = format!("# WORKFLOW CONTEXT (Step {} of workflow):\n\n", workflow_state.step_results.len() + 1);

                if !workflow_state.step_results.is_empty() {
                    workflow_msg.push_str("## Previous Steps:\n");
                    for (i, step_result) in workflow_state.step_results.iter().enumerate() {
                        let status_icon = if step_result.success { "✅" } else { "❌" };
                        workflow_msg.push_str(&format!(
                            "- Step {}: {} {} {}\n",
                            i + 1,
                            step_result.step_id,
                            status_icon,
                            step_result.error.as_ref().unwrap_or(&"Completed".to_string())
                        ));
                    }
                    workflow_msg.push_str("\n");
                }

                if !workflow_state.shared_context.is_empty() {
                    workflow_msg.push_str("## Shared Context:\n");
                    for (key, value) in &workflow_state.shared_context {
                        workflow_msg.push_str(&format!("- {}: {}\n", key, value.chars().take(100).collect::<String>()));
                    }
                    workflow_msg.push_str("\n");
                }

                messages.push(ChatMessage::user(workflow_msg));
            } else {
                // Fallback to raw JSON if parsing fails
                messages.push(ChatMessage::user(format!("# WORKFLOW CONTEXT:\n{}",
                    serde_json::to_string_pretty(workflow_state_value).unwrap_or_default())));
            }
        }

        // Add step parameters
        if !step.parameters.is_empty() {
            let mut params_msg = format!("# STEP PARAMETERS (from {} definition):\n", step.name);

            // Format parameters with better readability
            for (key, value) in &step.parameters {
                let value_str = match value {
                    serde_json::Value::String(s) => format!("\"{}\"", s),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Array(arr) => format!("{} items", arr.len()),
                    serde_json::Value::Object(obj) => format!("{} fields", obj.len()),
                    serde_json::Value::Null => "null".to_string(),
                };

                params_msg.push_str(&format!("- {}: {}\n", key, value_str));
            }

            messages.push(ChatMessage::user(params_msg));
        }
        // Add Tools instructions per tool type
        if let Some(tools) = tools{

            let mut tools_instructions = HashSet::new();
            let mut accumulated_instructions: String = "".to_string();
            for tool_name in &step.required_tools {
                if let Some(tool_instruction) = tools.get_tool_type_instructions(tool_name){
                    if tools_instructions.insert(tool_instruction.clone()){
                        accumulated_instructions += &format!("# RELEVANT TOOLS USAGE CONTEXT :\n");
                        accumulated_instructions += &tool_instruction ;
                    }
                };
            };
            messages.push(ChatMessage::user(accumulated_instructions.clone()));

            debug!(target: "agent_execution", "Accumulated Tool instructions: {} for step: {}",accumulated_instructions, step.name);
        }


        // Add RAG context if available
        if let Some(rag_context) = &context.rag_context {
            // Count sources and estimate relevance from context content
            let source_count = rag_context.matches("##").count();
            let context_length = rag_context.len();
            let estimated_tokens = context_length / 4;

            let header = if source_count > 0 {
                format!("# RELEVANT CONTEXT ({} sources, ~{} tokens):\n{}",
                    source_count, estimated_tokens, rag_context)
            } else {
                format!("# RELEVANT CONTEXT (~{} tokens):\n{}",
                    estimated_tokens, rag_context)
            };

            messages.push(ChatMessage::user(header));
        }

        // Add history context if available (for cases where it's separate from RAG)
        if let Some(history_context) = &context.history_context {
            // Estimate the amount of history content
            let estimated_tokens = history_context.len() / 4;
            let section_count = history_context.matches("##").count();

            let header = if section_count > 0 {
                format!("# CONVERSATION HISTORY ({} sections, ~{} tokens):\n{}",
                    section_count, estimated_tokens, history_context)
            } else {
                format!("# CONVERSATION HISTORY (~{} tokens):\n{}",
                    estimated_tokens, history_context)
            };

            messages.push(ChatMessage::user(header));
        }

        // Add main user prompt
        messages.push(ChatMessage::user(format!("# USER PROMPT:\n{}", context.description)));

        // Record LLM messages as span attributes for Jaeger visibility
        current_span.record("llm.messages_sent", messages.len());
        let messages_json = serde_json::to_string(&messages.iter().map(|m| {
            serde_json::json!({"role": format!("{:?}", m.role), "content": m.content})
        }).collect::<Vec<_>>()).unwrap_or_default();
        current_span.record("llm.messages_json", messages_json.as_str());

        // Also log for console debugging
        debug!(target: "agent_execution", "Sending {} messages to LLM", messages.len());
        for (i, msg) in messages.iter().enumerate() {
            debug!(target: "agent_execution", "Message {}: Role={:?}, Content length: {}", i + 1, msg.role, msg.content.len());
        }
        messages
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
    fn client(&self) -> &Ollama;

    // Execute with type-erased result
    async fn execute(&self, context: AgentContext) -> Result<AgentResult>;
}

// Blanket implementation: any TypedAgent automatically becomes an Agent
#[async_trait]
impl<T> Agent for T
where
    T: TypedAgent,
{
    fn id(&self) -> &str { TypedAgent::id(self) }
    fn agent_type(&self) -> AgentType { TypedAgent::agent_type(self) }
    fn system_prompt(&self) -> &str { TypedAgent::system_prompt(self) }
    fn model(&self) -> &str { TypedAgent::model(self) }
    fn client(&self) -> &Ollama { TypedAgent::client(self) }

    #[instrument(name = "agent_workflow_execution", skip(self, context), fields(agent_id = %self.id(), agent_type = %self.agent_type()))]
    async fn execute(&self, context: AgentContext) -> Result<AgentResult> {
        // All agents use workflow execution
        let workflow_steps = self.define_workflow_steps(&context);
        let tools = ToolSet::new(&context.clone().project_scope.unwrap().root);
        self.execute_workflow(context, workflow_steps, tools).await
    }

}

/// Context passed to agents during execution
#[derive(Debug, Display, Clone)]
#[display("AgentContext: {description}")]
pub struct AgentContext {
    /// Primary task description
    pub description: String,

    /// Context dependencies that must be satisfied
    pub dependencies: Vec<String>,

    /// Results from dependency tasks
    pub dependency_outputs: HashMap<String, Value>,
    /// Conversation identifier
    pub conversation_id: Option<ConversationId>,

    /// Project scope information
    pub project_scope: Option<ProjectScope>,
    /// Enhanced context from RAG system
    pub rag_context: Option<String>,

    /// Historical context
    pub history_context: Option<String>,

    /// Additional metadata
    pub metadata: HashMap<String, Value>,
}

impl AgentContext {
    /// Create new context with minimal required fields
    pub fn new(description: String, conversation_id: String) -> Self {
        Self {
            description,
            dependencies: Vec::new(),
            dependency_outputs: HashMap::new(),
            conversation_id: Some(ConversationId(conversation_id)),
            project_scope: None,
            rag_context: None,
            history_context: None,
            metadata: HashMap::new(),
        }
    }

    /// Estimate total tokens for this context
    pub fn estimate_tokens(&self) -> usize {
        let mut total = 0;
        total += self.description.len() / 4;

        if let Some(rag) = &self.rag_context {
            total += rag.len() / 4;
        }

        if let Some(history) = &self.history_context {
            total += history.len() / 4;
        }

        total.max(1)
    }

    /// Set dependency outputs
    pub fn with_dependency_outputs(mut self, outputs: HashMap<String, Value>) -> Self {
        self.dependency_outputs = outputs;
        self
    }

    /// Set RAG context
    pub fn with_rag_context(mut self, context: String) -> Self {
        self.rag_context = Some(context);
        self
    }

    /// Set project scope
    pub fn with_project_scope(mut self, scope: ProjectScope) -> Self {
        self.project_scope = Some(scope);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

}
#[cfg(test)]
mod tests {
    // Uncomment when tests are needed
    // use super::*;

    // Add tests here when needed
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
//     fn test_agent_context_token_estimation() {
//         let ctx = AgentContext::new("test".to_string(), "conv1".to_string());
//         assert!(ctx.estimate_tokens() > 0);
//     }
// }

