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
        let system_prompt = PlanningAgent::build_system_prompt(&system_prompt);
        Self {
            id,
            client,
            system_prompt,
            model,
            temperature: temperature.clamp(0.0, 2.0),
            max_tokens,
        }
    }

    fn build_system_prompt(system_prompt: &str) -> String {
        let usage = r#"
        ## WORKFLOW:
            1. Analyse your given context (RAG, History, user prompt) and understand which files of the working_directory are important for the task
            2. Get a list of files via the filesystem tool and generate a mental model of the underlying structure.
            3. Assess the complexity level from the provided analysis and determine appropriate task count
            4. Decompose the task into relevant subtasks ONLY if necessary, assign dependencies and give detailed descriptions

        ## COMPLEXITY-BASED TASK GUIDELINES:
        You will receive a complexity analysis. Use it to determine task decomposition:
            - **Moderate**: Prefer 1-2 tasks maximum. Only split if genuinely independent components exist.
            - **Complex**: 2-3 tasks maximum. Split into logical phases or components.
            - **VeryComplex**: 3+ tasks allowed. Break down into clear subsystems or phases.

        IMPORTANT: Favor fewer tasks over many. Each task should be substantial and meaningful.

        ## CRITICAL TOOLS USAGE RULES:
            - You are only allowed to use the "list" function of the filesystem tool. Do NOT use other functions of this tool.

        ## CRITICAL RULES FOR DEPENDENCIES:
            1. The entries of a subtasks dependencies MUST match actual subtask ids and agent_type of the task you're depending on.
            2. Use the exact agent types from the available_agents list provided to you.
            3. If task 'task-2' depends on task 'task-1', write: 'dependencies': ['task-1']

            ## Examples by Complexity:

            MODERATE (prefer single task):
            {
              'subtasks': [
                {'id': 'task-1', 'agent_type': '<agent_type>', 'description': 'Complete implementation including all components', 'dependencies': []}
              ]
            }
            
            COMPLEX (2-3 tasks if truly needed):
            {
              'subtasks': [
                {'id': 'task-1', 'agent_type': '<agent_type_1>', 'description': 'Core foundation and data structures', 'dependencies': []},
                {'id': 'task-2', 'agent_type': '<agent_type_2>', 'description': 'Main business logic using foundation', 'dependencies': ['task-1']}
              ]
            }"#;

        format!("##{}\n{}", system_prompt, usage)
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

}
