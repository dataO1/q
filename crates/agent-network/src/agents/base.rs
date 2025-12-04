//! Base agent trait and context types
//!
//! Defines the core Agent trait that all specialized agents implement,
//! along with context types for passing information to agents.

use ai_agent_common::{AgentType, ConversationId, ProjectScope, StatusEvent, EventSource, EventType};
use async_trait::async_trait;
use derive_more::Display;
use async_openai::{
    config::OpenAIConfig, types::{
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestDeveloperMessageContent, ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage, ChatCompletionRequestSystemMessageContent, ChatCompletionRequestToolMessageArgs, ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent, ChatCompletionResponseMessage, ChatCompletionTool, ChatCompletionToolChoiceOption, CreateChatCompletionRequest, CreateChatCompletionRequestArgs, CreateChatCompletionResponse, ResponseFormat, ResponseFormatJsonSchema, Role
    }, Client
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Mutex, broadcast};
use tracing::{debug, info, error, warn, instrument, Instrument};
use std::{collections::{HashMap, HashSet}, sync::Arc};
use chrono::{self, Duration};
use crate::{execution_manager::BidirectionalEventChannel, hitl::ApprovalDecision};
use schemars::JsonSchema;
use anyhow::{Context, Result, anyhow};

use crate::{
    agents::AgentResult,
    tools::{ToolResult, ToolSet, ToolExecution},
    hitl::{RiskAssessment, AuditLogger, AuditEvent},
};
use ai_agent_common::RiskLevel;


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
    fn client(&self) -> &Client<OpenAIConfig>;

    /// Define the workflow steps for this agent
    /// Each agent can define its own sequence of steps, each either OneShot or ReAct
    fn define_workflow_steps(&self, context: &AgentContext) -> Vec<WorkflowStep>;

    /// Estimate token count (rough: 1 token ≈ 4 characters)
    fn estimate_tokens(text: &str) -> usize {
        (text.len() / 4).max(1)
    }

    /// Extract content from a ChatCompletionRequestMessage
    fn extract_message_content(message: &ChatCompletionRequestMessage) -> String {
        match message {
            ChatCompletionRequestMessage::System(msg) => {
                match &msg.content {
                    ChatCompletionRequestSystemMessageContent::Text(text) => text.clone(),
                    ChatCompletionRequestSystemMessageContent::Array(_) => {
                        "[Complex content with multiple parts]".to_string()
                    }
                }
            }
            ChatCompletionRequestMessage::User(msg) => {
                match &msg.content {
                    ChatCompletionRequestUserMessageContent::Text(text) => text.clone(),
                    ChatCompletionRequestUserMessageContent::Array(_) => {
                        "[Complex content with multiple parts]".to_string()
                    }
                }
            }
            ChatCompletionRequestMessage::Assistant(msg) => {
                match &msg.content {
                    Some(ChatCompletionRequestAssistantMessageContent::Text(text)) => text.clone(),
                    Some(ChatCompletionRequestAssistantMessageContent::Array(_)) => {
                        "[Complex content with multiple parts]".to_string()
                    }
                    None => "[No content]".to_string(),
                }
            }
            ChatCompletionRequestMessage::Tool(msg) => {
                match &msg.content {
                    ChatCompletionRequestToolMessageContent::Text(text) => text.clone(),
                    ChatCompletionRequestToolMessageContent::Array(_) => {
                        "[Complex content with multiple parts]".to_string()
                    }
                }
            }
            ChatCompletionRequestMessage::Function(msg) => {
                msg.content.as_ref().map(|c| c.clone()).unwrap_or_else(|| "[No content]".to_string())
            }
            ChatCompletionRequestMessage::Developer(msg) => {
                match &msg.content {
                    ChatCompletionRequestDeveloperMessageContent::Text(text) => text.clone(),
                    ChatCompletionRequestDeveloperMessageContent::Array(_) => {
                        "[Complex content with multiple parts]".to_string()
                    }
                }
            }
        }
    }

    /// Execute workflow steps sequentially
    async fn execute_workflow(
        &self,
        context: AgentContext,
        workflow_steps: Vec<WorkflowStep>,
        tools: Arc<ToolSet>,
        event_channel: BidirectionalEventChannel,
        audit_logger: Option<Arc<AuditLogger>>,
    ) -> Result<AgentResult> {
        // Emit agent started event
        let conversation_id = context.conversation_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let agent_started_event = StatusEvent {
            conversation_id: conversation_id.clone(),
            timestamp: chrono::Utc::now(),
            source: EventSource::Agent {
                agent_id: self.id().to_string(),
                agent_type: self.agent_type(),
                task_id: context.task_id.clone(),
            },
            event: EventType::AgentStarted {
                context_size: context.description.len() +
                    context.rag_context.as_ref().map(|c| c.len()).unwrap_or(0) +
                    context.history_context.as_ref().map(|c| c.len()).unwrap_or(0)
            },
        };

        if let Err(_) = event_channel.send(agent_started_event).await {
            debug!("Failed to send agent started event");
        }

        let mut workflow_state = WorkflowState::new();
        let mut final_result = None;
        let mut all_tool_executions = Vec::new();

        for (step_index, step) in workflow_steps.iter().enumerate() {
            debug!("Executing workflow step {}/{}: {}", step_index + 1, workflow_steps.len(), step.name);

            // Emit workflow step started event
            let step_started_event = StatusEvent {
                conversation_id: conversation_id.clone(),
                timestamp: chrono::Utc::now(),
                source: EventSource::Agent {
                    agent_id: self.id().to_string(),
                    agent_type: self.agent_type(),
                    task_id: context.task_id.clone(),
                },
                event: EventType::WorkflowStepStarted {
                    step_name: step.name.clone()
                },
            };

            if let Err(_) = event_channel.send(step_started_event).await {
                debug!("Failed to send workflow step started event");
            }

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
                    self.execute_step_react(&updated_context, step, Arc::clone(&tools), *max_iterations, &event_channel, &audit_logger).await
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

                    // Emit workflow step completed event
                    let step_completed_event = StatusEvent {
                        conversation_id: conversation_id.clone(),
                        timestamp: chrono::Utc::now(),
                        source: EventSource::Agent {
                            agent_id: self.id().to_string(),
                            agent_type: self.agent_type(),
                            task_id: context.task_id.clone(),
                        },
                        event: EventType::WorkflowStepCompleted {
                            step_name: step.name.clone()
                        },
                    };

                    if let Err(_) = event_channel.send(step_completed_event).await {
                        debug!("Failed to send workflow step completed event");
                    }
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

                    // Emit agent failed event
                    let agent_failed_event = StatusEvent {
                        conversation_id: conversation_id.clone(),
                        timestamp: chrono::Utc::now(),
                        source: EventSource::Agent {
                            agent_id: self.id().to_string(),
                            agent_type: self.agent_type(),
                            task_id: context.task_id.clone(),
                        },
                        event: EventType::AgentFailed {
                            error: error_msg.clone()
                        },
                    };

                    if let Err(_) = event_channel.send(agent_failed_event).await {
                        debug!("Failed to send agent failed event");
                    }

                    return Err(anyhow!(error_msg));
                }
            }
        }
        let final_result = serde_json::from_str(&final_result.unwrap_or_default());

        // Create final agent result combining all workflow steps
        let agent_result = AgentResult {
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
        };

        // Emit agent completed event
        let agent_completed_event = StatusEvent {
            conversation_id: conversation_id.clone(),
            timestamp: chrono::Utc::now(),
            source: EventSource::Agent {
                agent_id: self.id().to_string(),
                agent_type: self.agent_type(),
                task_id: context.task_id.clone(),
            },
            event: EventType::AgentCompleted {
                result: agent_result.reasoning.clone().unwrap_or_default()
            },
        };

        if let Err(_) = event_channel.send(agent_completed_event).await {
            debug!("Failed to send agent completed event");
        }

        Ok(agent_result)
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

        // Execute LLM call - build request with optional structured output
        let request = if step.formatted {
            let json_schema = schemars::schema_for!(Self::Output);
            let schema_value = serde_json::to_value(json_schema).unwrap_or_default();
            CreateChatCompletionRequestArgs::default()
                .model(self.model())
                .messages(messages.clone())
                .response_format(ResponseFormat::JsonSchema {
                    json_schema: ResponseFormatJsonSchema {
                        name: "structured_response".to_string(),
                        description: Some("Structured response following the provided schema".to_string()),
                        schema: Some(schema_value),
                        strict: Some(true),
                    }
                })
                .build()?
        } else {
            CreateChatCompletionRequestArgs::default()
                .model(self.model())
                .messages(messages.clone())
                .build()?
        };

        debug!(target: "agent_execution", "Starting LLM call for OneShot step '{}' (model: {}, messages: {}, formatted: {})",
            step.name, self.model(), messages.len(), step.formatted);

        // Execute LLM call with enhanced span instrumentation and real-time events
        let content_strings: Vec<String> = messages.iter().map(|m| Self::extract_message_content(m)).collect();
        let prompt_tokens = Self::estimate_tokens(&content_strings.join("\n"));

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
            let response = self.client().chat().create(request).await?;
            let duration = start_time.elapsed();

            // Extract content from response
            let default_content = String::new();
            let content = response.choices.first()
                .and_then(|choice| choice.message.content.as_ref())
                .unwrap_or(&default_content);

            // Calculate completion metrics immediately
            let completion_tokens = Self::estimate_tokens(content);
            let latency_per_token = if completion_tokens > 0 {
                duration.as_millis() / completion_tokens as u128
            } else { 0 };

            // Record response received event with details
            info!(target: "llm_inference", "llm_response_received: completion_tokens={}, total_latency_ms={}, latency_per_token_ms={}, response_length={}",
                completion_tokens, duration.as_millis(), latency_per_token, content.len());

            // Record completion metrics in span
            let current_span = tracing::Span::current();
            current_span.record("llm.token_count.completion", &completion_tokens);
            current_span.record("llm.latency_per_token", &format!("{}ms", latency_per_token));

            info!("LLM inference completed for OneShot step '{}' (response: {} chars, completion tokens: {}, latency: {}ms)",
                step.name, content.len(), completion_tokens, duration.as_millis());

            Ok::<_, anyhow::Error>(response)
        }.instrument(llm_span).await?;

        // Extract final content
        let final_content = response.choices.first()
            .and_then(|choice| choice.message.content.as_ref())
            .unwrap_or(&String::new())
            .clone();

        // Record LLM response and results as span attributes for Jaeger visibility
        current_span.record("llm.response_content", final_content.as_str());
        current_span.record("result.success", true);
        current_span.record("result.output_length", final_content.len());

        debug!(target: "agent_execution", "LLM call completed for OneShot step '{}' (response length: {} chars)",
            step.name, final_content.len());
        debug!(target: "agent_execution", "Response preview: {}",
            final_content.chars().take(200).collect::<String>());

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
            output: Some(final_content),
            error: None,
            tool_executions: vec![], // OneShot doesn't use tools
        };

        debug!(target: "agent_execution", "OneShot step '{}' completed successfully", step.name);

        Ok(step_result)
    }

    /// Assess risk level for a tool call and determine if HITL approval is needed
    fn assess_tool_risk(&self, tool_name: &str, confidence: f32) -> (RiskLevel, bool) {
        // Hardcoded risk levels for now - can be made configurable later
        let tool_risk = match tool_name {
            // Critical risk tools - always require approval
            "delete_file" | "DeleteFileTool" => RiskLevel::Critical,
            // High risk tools - require approval if confidence < 0.8
            "write_file" | "WriteFileTool" | "create_directory" | "CreateDirectoryTool" => RiskLevel::High,
            // Medium risk tools - require approval if confidence < 0.6
            "read_file" | "ReadFileTool" | "list_directory" | "ListDirectoryTool" => RiskLevel::Medium,
            // Low risk tools - generally safe
            "file_exists" | "FileExistsTool" | "file_metadata" | "FileMetadataTool" => RiskLevel::Low,
            // Unknown tools default to high risk
            _ => RiskLevel::High,
        };

        let needs_approval = match tool_risk {
            RiskLevel::Critical => true, // Always require approval for critical operations
            RiskLevel::High => confidence < 0.8, // Require approval if not confident
            RiskLevel::Medium => confidence < 0.6, // Require approval if low confidence
            RiskLevel::Low => false, // Generally safe
        };

        (tool_risk, needs_approval)
    }

    /// Request HITL approval for a tool call
    async fn request_hitl_approval(
        &self,
        tool_name: &str,
        tool_args: &str,
        agent_context: &AgentContext,
        event_channel: &BidirectionalEventChannel,
        risk_level: RiskLevel,
    ) -> Result<ApprovalDecision> {
        let request_id = format!("hitl_{}_{}",
            agent_context.task_id.as_ref().unwrap_or(&"unknown".to_string()),
            chrono::Utc::now().timestamp_millis()
        );

        // Create risk assessment for HITL request
        let agent_result = AgentResult {
            agent_id: self.id().to_string(),
            output: serde_json::json!({}),
            confidence: 0.5, // Requesting approval indicates low confidence
            requires_hitl: true,
            tokens_used: None,
            reasoning: Some(format!("Tool {} requires human approval due to {:?} risk", tool_name, risk_level)),
            tool_executions: vec![],
        };

        let risk_assessment = RiskAssessment::new(&agent_result, self.agent_type(),
            Some(format!("Tool {} requires approval: risk={:?}, args={}", tool_name, risk_level, tool_args)));

        // Send HITL requested event to notify TUI
        let hitl_event = StatusEvent {
            conversation_id: agent_context.conversation_id
                .as_ref()
                .map(|id| id.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            timestamp: chrono::Utc::now(),
            source: EventSource::Agent {
                agent_id: self.id().to_string(),
                agent_type: self.agent_type(),
                task_id: agent_context.task_id.clone(),
            },
            event: EventType::HitlRequested {
                risk_level: format!("{:?}", risk_level),
                task_description: format!("{}: {} with args: {}", self.agent_type(), tool_name, tool_args),
            },
        };

        if let Err(_) = event_channel.send(hitl_event).await {
            warn!("Failed to send HITL requested event");
        }


        // Create audit event for the request
        let audit_event = AuditEvent {
            event_id: request_id.clone(),
            timestamp: chrono::Utc::now(),
            agent_id: self.id().to_string(),
            task_id: agent_context.task_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            action: format!("HITL_REQUEST:{}", tool_name),
            risk_level: format!("{:?}", risk_level),
            decision: "PENDING".to_string(),
            metadata: [
                ("tool_name".to_string(), tool_name.to_string()),
                ("tool_args".to_string(), tool_args.to_string()),
                ("agent_type".to_string(), format!("{:?}", self.agent_type())),
            ].into(),
        };

        AuditLogger::log(audit_event);

        info!("HITL approval requested for {} tool {} (risk: {:?})",
              self.agent_type(), tool_name, risk_level);
        // Step 2: Wait for HITL decision from client (inbound: client → server)
        let event_key = format!("hitl_decision:{}", request_id);
        let timeout = std::time::Duration::from_secs(3000) ; // 5 minutes

        let event = event_channel.wait_for(event_key, timeout).await
            .context("Timeout or error waiting for HITL decision")?;

        let decision = match event {
            // TODO: check or receive only message for the respective id
            StatusEvent{ conversation_id, timestamp, source, event: EventType::HitlDecision{ id, approved, modified_content, reasoning } } =>{
                let decision = if approved{ApprovalDecision::Approved{reasoning}}else{ApprovalDecision::Rejected{reasoning}};

                // Send completion event
                let completion_event = StatusEvent {
                    conversation_id: agent_context.conversation_id
                        .as_ref()
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    timestamp: chrono::Utc::now(),
                    source: EventSource::Agent {
                        agent_id: self.id().to_string(),
                        agent_type: self.agent_type(),
                        task_id: agent_context.task_id.clone(),
                    },
                    event: EventType::HitlCompleted {
                        approved: matches!(decision, ApprovalDecision::Approved{reasoning: None}),
                        reason: Some(format!("Policy decision for {:?} risk: {:?}", risk_level, decision)),
                    },
                };

                if let Err(_) = event_channel.send(completion_event).await {
                    warn!("Failed to send HITL completed event");
                }
                decision
            },
            _ =>{ApprovalDecision::Approved{reasoning: None}}
        };

        // // Apply approval policy based on risk level and business rules
        // let decision = match risk_level {
        //     RiskLevel::Critical => {
        //         warn!("CRITICAL OPERATION BLOCKED: {} tool {} requires manual approval",
        //               self.agent_type(), tool_name);
        //         // Critical operations always require approval - block for safety
        //         ApprovalDecision::Rejected
        //     },
        //     RiskLevel::High => {
        //         warn!("HIGH-RISK OPERATION: {} tool {} requires review",
        //               self.agent_type(), tool_name);
        //         // High-risk operations - for now auto-approve but log for review
        //         // In production, this would wait for human decision via API
        //         ApprovalDecision::Approved
        //     },
        //     RiskLevel::Medium => {
        //         info!("MEDIUM-RISK OPERATION: {} tool {} auto-approved with monitoring",
        //               self.agent_type(), tool_name);
        //         ApprovalDecision::Approved
        //     },
        //     RiskLevel::Low => {
        //         debug!("LOW-RISK OPERATION: {} tool {} auto-approved",
        //                self.agent_type(), tool_name);
        //         ApprovalDecision::Approved
        //     },
        // };

        // Log the final decision
        let decision_audit = AuditEvent {
            event_id: format!("{}_decision", request_id),
            timestamp: chrono::Utc::now(),
            agent_id: self.id().to_string(),
            task_id: agent_context.task_id.as_ref().unwrap_or(&"unknown".to_string()).clone(),
            action: format!("HITL_DECISION:{}", tool_name),
            risk_level: format!("{:?}", risk_level),
            decision: format!("{:?}", decision),
            metadata: [
                ("tool_name".to_string(), tool_name.to_string()),
                ("tool_args".to_string(), tool_args.to_string()),
                ("decision_reason".to_string(), format!("Auto-policy for {:?} risk level", risk_level)),
            ].into(),
        };

        AuditLogger::log(decision_audit);

        info!("HITL decision for {} tool {}: {:?}", self.agent_type(), tool_name, decision);
        Ok(decision)
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
        tools: Arc<ToolSet>,
        max_iterations: Option<usize>,
        event_channel: &BidirectionalEventChannel,
        audit_logger: &Option<Arc<AuditLogger>>,
    ) -> Result<StepResult> {
        // Record comprehensive input details as span attributes for Jaeger visibility
        let current_span = tracing::Span::current();
        current_span.record("step.description", step.description.as_str());
        current_span.record("context.description", context.description.as_str());

        current_span.record("Task description:",&context.description);
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
        let mut messages = self.build_initial_message(context,step, Some(&tools));

        // Convert ToolSet tools to ChatCompletionTool format
        let openai_tools = tools.to_openai_tools(&step.required_tools)?;

        debug!(target: "agent_execution", "Starting streaming ReAct step '{}'", step.name);
        if step.required_tools.is_empty() {
            info!(target: "agent_execution", "ReAct step '{}' with no tools", step.name);
        } else {
            info!(target: "agent_execution", "ReAct step '{}' with tools: {}", step.name, step.required_tools.join(", "));
        }

        // Execute ReAct loop with streaming
        let max_iter = max_iterations.unwrap_or(5);
        let mut tool_executions = Vec::new();
        let mut final_response = String::new();

        for iteration in 0..max_iter {
            debug!(target: "agent_execution", "ReAct iteration {}/{} for step '{}'", iteration + 1, max_iter, step.name);

            // Build request for this iteration
            let request = if step.formatted {
                let json_schema = schemars::schema_for!(Self::Output);
                let schema_value = serde_json::to_value(json_schema).unwrap_or_default();
                if !openai_tools.is_empty() {
                    CreateChatCompletionRequestArgs::default()
                        .model(self.model())
                        .messages(messages.clone())
                        .response_format(ResponseFormat::JsonSchema {
                            json_schema: ResponseFormatJsonSchema {
                                name: "structured_response".to_string(),
                                description: Some("Structured response following the provided schema".to_string()),
                                schema: Some(schema_value),
                                strict: Some(true),
                            }
                        })
                        .tools(openai_tools.clone())
                        .tool_choice(ChatCompletionToolChoiceOption::Auto)  // ✅ ADD
                        .build()?
                } else {
                    CreateChatCompletionRequestArgs::default()
                        .model(self.model())
                        .messages(messages.clone())
                        .response_format(ResponseFormat::JsonSchema {
                            json_schema: ResponseFormatJsonSchema {
                                name: "structured_response".to_string(),
                                description: Some("Structured response following the provided schema".to_string()),
                                schema: Some(schema_value),
                                strict: Some(true),
                            }
                        })
                        .build()?
                }
            } else {
                if !openai_tools.is_empty() {
                    CreateChatCompletionRequestArgs::default()
                        .model(self.model())
                        .messages(messages.clone())
                        .tools(openai_tools.clone())
                        .tool_choice(ChatCompletionToolChoiceOption::Auto)  // ✅ ADD
                        .build()?
                } else {
                    CreateChatCompletionRequestArgs::default()
                        .model(self.model())
                        .messages(messages.clone())
                        .build()?
                }
            };
            let response = self.client().chat().create(request).await?;

            if let Some(choice) = response.choices.first() {
                // Handle text response
                if let Some(content) = &choice.message.content {
                    final_response.push_str(content);
                    debug!(target: "agent_execution", "Received text response: {}", content.chars().take(100).collect::<String>());
                }

                // Handle tool calls
                if let Some(tool_calls) = &choice.message.tool_calls {
                    debug!(target: "agent_execution", "Received {} tool calls", tool_calls.len());

                    for tool_call in tool_calls {
                        let function = &tool_call.function;

                        // HITL: Check if this tool requires approval
                        // Use agent's current confidence - for now, estimate from iteration count
                        let estimated_confidence = match iteration {
                            0 => 0.9, // High confidence on first attempt
                            1 => 0.7, // Medium confidence on second attempt
                            2 => 0.5, // Lower confidence on third attempt
                            _ => 0.3, // Low confidence after multiple attempts
                        };

                        let (risk_level, needs_approval) = self.assess_tool_risk(&function.name, estimated_confidence);

                        // if needs_approval {
                        if true{ //TODO: remove, this is just for debugging HITL
                            debug!(target: "agent_execution", "Tool {} requires HITL approval (risk: {:?})", function.name, risk_level);

                            // Add assistant message showing the tool call the agent wants to make
                            messages.push(ChatCompletionRequestMessage::Assistant(
                                async_openai::types::ChatCompletionRequestAssistantMessageArgs::default()
                                    .content(format!("I want to call {} with arguments: {}", function.name, function.arguments))
                                    .tool_calls(vec![tool_call.clone()])
                                    .build()?
                            ));

                            match self.request_hitl_approval(
                                &function.name,
                                &function.arguments,
                                context,
                                event_channel,
                                risk_level,
                            ).await? {
                                ApprovalDecision::Approved{reasoning} => {
                                    debug!(target: "agent_execution", "HITL approved tool execution: {}", function.name);
                                    if let Some(reasoning) = reasoning{
                                        // Add tool result to conversation
                                        messages.push(ChatCompletionRequestUserMessage::from(format!("## HITL Feedback:\n{}",reasoning)).into());
                                    }
                                    // Execute tool as normal
                                    let tool_execution = tools.execute_tool(&function.name, &function.arguments).await?;
                                    tool_executions.push(tool_execution.clone());

                                    // Add tool result to conversation
                                    messages.push(ChatCompletionRequestMessage::Tool(
                                        async_openai::types::ChatCompletionRequestToolMessageArgs::default()
                                            .content(tool_execution.result.output)
                                            .tool_call_id(tool_call.id.clone())
                                            .build()?
                                    ));
                                }
                                ApprovalDecision::Rejected{reasoning} => {
                                    warn!(target: "agent_execution", "HITL rejected tool execution: {}", function.name);
                                    // Add rejection message to conversation
                                    messages.push(ChatCompletionRequestMessage::Tool(
                                        async_openai::types::ChatCompletionRequestToolMessageArgs::default()
                                            .content(format!("Tool execution was rejected by human reviewer. Please try a different approach or ask for guidance."))
                                            .tool_call_id(tool_call.id.clone())
                                            .build()?
                                    ));

                                    if let Some(reasoning) = reasoning{
                                        // Add tool result to conversation
                                        messages.push(ChatCompletionRequestUserMessage::from(format!("## HITL Feedback:\n{}",reasoning)).into());
                                    }
                                }
                                ApprovalDecision::NeedsMoreInfo => {
                                    info!(target: "agent_execution", "HITL requested more info for tool: {}", function.name);
                                    // Add request for more info to conversation
                                    messages.push(ChatCompletionRequestMessage::Tool(
                                        async_openai::types::ChatCompletionRequestToolMessageArgs::default()
                                            .content(format!("Human reviewer needs more information about this action. Please provide more context about why you want to {} and what you expect to happen.", function.name))
                                            .tool_call_id(tool_call.id.clone())
                                            .build()?
                                    ));
                                }
                            }
                        } else {
                            // No approval needed, execute directly
                            debug!(target: "agent_execution", "Tool {} auto-approved (risk: {:?})", function.name, risk_level);
                            let tool_execution = tools.execute_tool(&function.name, &function.arguments).await?;
                            tool_executions.push(tool_execution.clone());

                            // Add tool result to conversation
                            messages.push(ChatCompletionRequestMessage::Tool(
                                async_openai::types::ChatCompletionRequestToolMessageArgs::default()
                                    .content(tool_execution.result.output)
                                    .tool_call_id(tool_call.id.clone())
                                    .build()?
                            ));
                        }
                    }

                    // Messages are captured by closure, no need to update here
                } else {
                    // No tool calls, we're done
                    break;
                }
            }
        }

        current_span.record("total_tool_executions", tool_executions.len());
        current_span.record("final_response_length", final_response.len());

        debug!(target: "agent_execution", "ReAct step '{}' completed successfully with {} tool executions",
               step.name, tool_executions.len());

        let step_result = StepResult {
            step_id: step.id.clone(),
            success: true,
            output: Some(final_response),
            error: None,
            tool_executions,
        };

        Ok(step_result)
    }

    /// Execute a single ReAct workflow step
    #[instrument(name = "agent_react_step", skip(self, context, step), fields())]
    fn build_initial_message(&self,context: &AgentContext, step: &WorkflowStep, tools: Option<&Arc<ToolSet>>)-> Vec<ChatCompletionRequestMessage>{

        let current_span = tracing::Span::current();
        // Build messages for this specific step
        let mut messages: Vec<ChatCompletionRequestMessage> = vec![
        ];

        // step instructions
        messages.push(ChatCompletionRequestSystemMessage::from(format!("# STEP: {}\n{}\n\n# INSTRUCTIONS:\n{}",
            step.name,
            step.description,
            self.system_prompt()
        )).into());

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

            messages.push(ChatCompletionRequestSystemMessage::from(dependency_msg).into());
        }

        // // Add workflow state if available
        // if let Some(workflow_state_value) = context.metadata.get("workflow_state") {
        //     // Try to parse the workflow state for better formatting
        //     if let Ok(workflow_state) = serde_json::from_value::<WorkflowState>(workflow_state_value.clone()) {
        //         let mut workflow_msg = format!("# WORKFLOW CONTEXT (Step {} of workflow):\n\n", workflow_state.step_results.len() + 1);
        //
        //         if !workflow_state.step_results.is_empty() {
        //             workflow_msg.push_str("## Previous Steps:\n");
        //             for (i, step_result) in workflow_state.step_results.iter().enumerate() {
        //                 let status_icon = if step_result.success { "✅" } else { "❌" };
        //                 workflow_msg.push_str(&format!(
        //                     "- Step {}: {} {} {}\n",
        //                     i + 1,
        //                     step_result.step_id,
        //                     status_icon,
        //                     step_result.error.as_ref().unwrap_or(&"Completed".to_string())
        //                 ));
        //             }
        //             workflow_msg.push_str("\n");
        //         }
        //
        //         if !workflow_state.shared_context.is_empty() {
        //             workflow_msg.push_str("## Shared Context:\n");
        //             for (key, value) in &workflow_state.shared_context {
        //                 workflow_msg.push_str(&format!("- {}: {}\n", key, value.chars().take(100).collect::<String>()));
        //             }
        //             workflow_msg.push_str("\n");
        //         }
        //
        //         messages.push(ChatMessage::user(workflow_msg));
        //     } else {
        //         // Fallback to raw JSON if parsing fails
        //         messages.push(ChatMessage::user(format!("# WORKFLOW CONTEXT:\n{}",
        //             serde_json::to_string_pretty(workflow_state_value).unwrap_or_default())));
        //     }
        // }
        //
        // // Add step parameters
        // if !step.parameters.is_empty() {
        //     let mut params_msg = format!("# STEP PARAMETERS (from {} definition):\n", step.name);
        //
        //     // Format parameters with better readability
        //     for (key, value) in &step.parameters {
        //         let value_str = match value {
        //             serde_json::Value::String(s) => format!("\"{}\"", s),
        //             serde_json::Value::Number(n) => n.to_string(),
        //             serde_json::Value::Bool(b) => b.to_string(),
        //             serde_json::Value::Array(arr) => format!("{} items", arr.len()),
        //             serde_json::Value::Object(obj) => format!("{} fields", obj.len()),
        //             serde_json::Value::Null => "null".to_string(),
        //         };
        //
        //         params_msg.push_str(&format!("- {}: {}\n", key, value_str));
        //     }
        //
        //     messages.push(ChatMessage::user(params_msg));
        // }
        // Add Tools instructions per tool type
        if let Some(tools) = tools{

            let mut tools_instructions = HashSet::new();
            let mut accumulated_instructions: String = "".to_string();
            for tool_name in &step.required_tools {
                if let Some(tool_instruction) = tools.get_tool_type_instructions(tool_name){
                    if tools_instructions.insert(tool_instruction.clone()){
                        accumulated_instructions += &format!("# RELEVANT TOOLS USAGE INSTRUCTIONS :\n");
                        accumulated_instructions += &tool_instruction ;
                    }
                };
            };
            if accumulated_instructions != ""{
                messages.push(ChatCompletionRequestSystemMessage::from(accumulated_instructions.clone()).into());
            }

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

            messages.push(ChatCompletionRequestSystemMessage::from(header).into());
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

            messages.push(ChatCompletionRequestSystemMessage::from(header).into());
        }

        // Add main user prompt
        messages.push(ChatCompletionRequestUserMessage::from(format!("# USER PROMPT (YOUR MAIN TASK):\n{}", context.description)).into());

        // Record LLM messages as span attributes for Jaeger visibility
        current_span.record("llm.messages_sent", messages.len());
        let messages_json = serde_json::to_string(&messages.iter().map(|m| {
            let role = match m {
                ChatCompletionRequestMessage::System(_) => "system",
                ChatCompletionRequestMessage::User(_) => "user",
                ChatCompletionRequestMessage::Assistant(_) => "assistant",
                ChatCompletionRequestMessage::Tool(_) => "tool",
                ChatCompletionRequestMessage::Function(_) => "function",
                ChatCompletionRequestMessage::Developer(_) => "developer",
            };
            serde_json::json!({"role": role, "content": Self::extract_message_content(m)})
        }).collect::<Vec<_>>()).unwrap_or_default();
        current_span.record("llm.messages_json", messages_json.as_str());

        // Also log for console debugging
        debug!(target: "agent_execution", "Sending {} messages to LLM", messages.len());
        for (i, msg) in messages.iter().enumerate() {
            let role = match msg {
                ChatCompletionRequestMessage::System(_) => "system",
                ChatCompletionRequestMessage::User(_) => "user",
                ChatCompletionRequestMessage::Assistant(_) => "assistant",
                ChatCompletionRequestMessage::Tool(_) => "tool",
                ChatCompletionRequestMessage::Function(_) => "function",
                ChatCompletionRequestMessage::Developer(_) => "developer",
            };
            let content = Self::extract_message_content(msg);
            debug!(target: "agent_execution", "Message {}: Role={}, Content length: {}", i + 1, role, content.len());
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
    fn client(&self) -> &Client<OpenAIConfig>;

    // Execute with type-erased result
    async fn execute(
        &self,
        context: AgentContext,
        event_channel: BidirectionalEventChannel,
        audit_logger: Option<Arc<AuditLogger>>,
    ) -> Result<AgentResult>;

    /// Define the workflow steps for this agent (from TypedAgent)
    fn define_workflow_steps(&self, context: &AgentContext) -> Vec<WorkflowStep>;
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
    fn client(&self) -> &Client<OpenAIConfig> { TypedAgent::client(self) }

    #[instrument(name = "agent_workflow_execution", skip(self, context, event_channel, audit_logger), fields(agent_id = %self.id(), agent_type = %self.agent_type()))]
    async fn execute(
        &self,
        context: AgentContext,
        event_channel: BidirectionalEventChannel,
        audit_logger: Option<Arc<AuditLogger>>,
    ) -> Result<AgentResult> {
        // All agents use workflow execution
        let workflow_steps = self.define_workflow_steps(&context);
        let tools = Arc::new(ToolSet::new(&context.clone().project_scope.unwrap().root));
        self.execute_workflow(context, workflow_steps, tools, event_channel, audit_logger).await
    }

    fn define_workflow_steps(&self, context: &AgentContext) -> Vec<WorkflowStep> {
        TypedAgent::define_workflow_steps(self, context)
    }

}

/// Context passed to agents during execution
#[derive(Debug, Display, Clone)]
#[display("AgentContext: {description}")]
pub struct AgentContext {
    /// Primary task description
    pub description: String,

    /// Task identifier (None for planning agent)
    pub task_id: Option<String>,

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
    pub fn new(description: String, conversation_id: String, task_id: Option<String>) -> Self {
        Self {
            description,
            task_id,
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

