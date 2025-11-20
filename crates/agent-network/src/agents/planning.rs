//! Coding agent implementation using Rig LLM framework
//!
//! The coding agent specializes in generating, reviewing, and refactoring code.
//! It integrates with Rig for LLM calls and supports local Ollama models.

use crate::{ agents::{base::TypedAgent, AgentContext}, orchestrator::AgentCapability};
use ai_agent_common::{AgentType, ErrorRecoveryStrategy};
use async_trait::async_trait;
use ollama_rs::Ollama;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskDecompositionPlan {
    /// High-level strategy/reasoning
    pub reasoning: String,

    /// Estimated complexity
    pub complexity_assessment: String,

    /// Ordered list of subtasks
    pub subtasks: Vec<SubtaskSpec>,

    /// Critical path tasks
    pub critical_path: Vec<String>,

    /// Whether human review is needed
    pub requires_hitl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubtaskSpec {
    pub id: String,  // e.g., "task-1", "task-2"
    /// Human-readable description
    pub description: String,

    /// Agent type to execute this
    pub agent_type: AgentType,

    /// IDs of tasks that must complete first
    pub dependencies: Vec<String>,

    /// Whether this subtask needs human approval
    pub requires_approval: bool,
}


/// Coding agent for code generation and review
pub struct PlanningAgent {
    id: String,
    model: String,
    client: Ollama,
    system_prompt: String,
    temperature: f32,
    max_tokens: usize,
}

impl PlanningAgent {
    /// Create a new coding agent
    pub fn new(
        id: String,
        model: String,
        system_prompt: String,
        temperature: f32,
        max_tokens: usize,
        ollama_host: &str,
        ollama_port: u16,
    ) -> Self {
        let client = Ollama::new(ollama_host, ollama_port);
        Self {
            id,
            client,
            system_prompt,
            model,
            temperature: temperature.clamp(0.0, 2.0),
            max_tokens,
        }
    }

    /// Build the prompt for the LLM
    fn build_prompt(&self, context: &AgentContext) -> String {
        let mut prompt = format!("Task ID: {}\n\n", context.task_id);
        prompt.push_str(&format!("Task: {}\n\n", context.description));

        if !context.tool_results.is_empty() {
            prompt.push_str("## Available Information\n");
            for tool_result in &context.tool_results {
                if tool_result.success {
                    prompt.push_str(&format!("From {}: {}\n", tool_result.tool_name, tool_result.output));
                }
            }
            prompt.push_str("\n");
        }

        if let Some(rag_context) = &context.rag_context {
            prompt.push_str("## Relevant Code Examples\n");
            prompt.push_str(rag_context);
            prompt.push_str("\n\n");
        }

        prompt.push_str("Please provide your response in the following format:\n");
        prompt.push_str("1. Analysis\n");
        prompt.push_str("2. Implementation\n");
        prompt.push_str("3. Explanation\n");

        prompt
    }

    /// Estimate confidence based on prompt characteristics
    fn estimate_confidence(&self, output: &str) -> f32 {
        let has_code = output.contains("```") || output.contains("fn ");
        let is_complete = output.len() > 200;
        let has_explanation = output.to_lowercase().contains("explanation")
            || output.to_lowercase().contains("why")
            || output.to_lowercase().contains("because");

        let mut confidence: f32 = 0.6;
        if has_code {
            confidence += 0.2;
        }
        if is_complete {
            confidence += 0.1;
        }
        if has_explanation {
            confidence += 0.1;
        }

        confidence.min(0.99)
    }
}

#[async_trait]
impl TypedAgent for PlanningAgent {
    fn id(&self) -> &str { &self.id }
    fn agent_type(&self) -> AgentType { AgentType::Planning}
    fn system_prompt(&self) -> &str { &self.system_prompt }
    fn model(&self) -> &str { &self.model }
    fn temperature(&self) -> f32 { self.temperature }
    fn client(&self) -> &Ollama { &self.client }
    type Output = TaskDecompositionPlan;

    fn build_prompt(&self, context: &AgentContext) -> String {
        let mut parts = vec![format!("# Planning Task: {}", context.description)];

        if let Some(ref rag) = context.rag_context {
            parts.push(format!("\n## RAG Context:\n{}", rag));
        }

        if let Some(ref hist) = context.history_context {
            parts.push(format!("\n## History:\n{}", hist));
        }
        //
        parts.push("\n## Instructions:".to_string());
        parts.push("CRITICAL RULES FOR DEPENDENCIES:
        1. The entries of a subtasks dependencies MUST match actual subtask ids and agent_type of the task you're depending on.
        2. If a Coding task with id 'task-2' depends on a Coding task with id 'task-1' , write: 'dependencies': [ 'task-1']
        3. If a Writing task with id 'task-8' depends on a coding task with id 'task-2' and on a Planning task with id 'task-3' , write: 'dependencies': ['task-2','task-3']".to_string());

        parts.push("Example CORRECT:
        {
          'subtasks': [
            {'id': 'task-1', 'agent_type': 'Coding', 'description': 'task-1's description', 'dependencies': []},
            {'id': 'task-2', 'agent_type': 'Coding', 'description': 'task-2's description', 'dependencies': ['task-1']},
            {'id': 'task-3', 'agent_type': 'Testing', 'description': 'task-3's description', 'dependencies': ['task-1']}
          ]
        }".to_string());
        parts.join("\n")
    }
}
