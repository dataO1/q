//! Coding agent implementation using Rig LLM framework
//!
//! The coding agent specializes in generating, reviewing, and refactoring code.
//! It integrates with Rig for LLM calls and supports local Ollama models.

use crate::agents::base::TypedAgent;
use ai_agent_common::{AgentType, QualityStrategy};
use async_trait::async_trait;
use ollama_rs::Ollama;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

/// Evaluator structured output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EvaluatorOutput {
    pub score: f32,
    #[serde(default)]
    pub issues: Vec<String>,
    #[serde(default)]
    pub suggestions: Vec<String>,
    pub summary: String,
}

/// Coding agent for code generation and review
pub struct EvaluatorAgent {
    id: String,
    model: String,
    client: Ollama,
    system_prompt: String,
    temperature: f32,
    max_tokens: usize,
    quality_strategy: QualityStrategy
}

impl EvaluatorAgent {
    /// Create a new coding agent
    pub fn new(
        id: String,
        model: String,
        system_prompt: String,
        temperature: f32,
        max_tokens: usize,
        quality_strategy: QualityStrategy,
        ollama_host: &str,
        ollama_port: u16,
    ) -> Self {
        let client = Ollama::new(ollama_host, ollama_port);
        Self {
            id,
            client,
            system_prompt,
            quality_strategy,
            model,
            temperature: temperature.clamp(0.0, 2.0),
            max_tokens,
        }
    }
}

#[async_trait]
impl TypedAgent for EvaluatorAgent {
    fn id(&self) -> &str { &self.id }
    fn agent_type(&self) -> AgentType { AgentType::Evaluator }
    fn system_prompt(&self) -> &str { &self.system_prompt }
    fn model(&self) -> &str { &self.model }
    fn temperature(&self) -> f32 { self.temperature }
    fn client(&self) -> &Ollama { &self.client }
    type Output = EvaluatorOutput;
    
    /// Define evaluator workflow steps with file reading logic
    fn define_workflow_steps(&self, context: &crate::agents::AgentContext) -> Vec<crate::agents::base::WorkflowStep> {
        use crate::agents::base::{WorkflowStep, StepExecutionMode};
        use std::collections::HashMap;
        
        // Extract files to evaluate from dependency tool executions
        let mut files_to_evaluate = Vec::new();
        for (_task_id, dep_output) in &context.dependency_outputs {
            if let Some(tool_executions) = dep_output.get("tool_executions") {
                if let Some(tool_array) = tool_executions.as_array() {
                    for tool_execution in tool_array {
                        if let Some(tool_name) = tool_execution.get("tool_name").and_then(|v| v.as_str()) {
                            if tool_name == "filesystem" {
                                if let Some(params) = tool_execution.get("parameters") {
                                    if let Some(command) = params.get("command").and_then(|v| v.as_str()) {
                                        if command == "write" {
                                            if let Some(file_path) = params.get("path").and_then(|v| v.as_str()) {
                                                files_to_evaluate.push(file_path.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Build file contents for evaluation
        let mut file_contents_param = HashMap::new();
        for file_path in files_to_evaluate {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    file_contents_param.insert(file_path.clone(), serde_json::Value::String(content));
                }
                Err(e) => {
                    file_contents_param.insert(
                        file_path.clone(), 
                        serde_json::Value::String(format!("Error reading file: {}", e))
                    );
                }
            }
        }

        let mut step_parameters = HashMap::new();
        let files_list = file_contents_param.keys().cloned().collect::<Vec<_>>();
        if !file_contents_param.is_empty() {
            step_parameters.insert("files_to_evaluate".to_string(), serde_json::Value::Object(file_contents_param.into_iter().collect()));
        }

        // Single step workflow for evaluator: analyze files from previous agents
        vec![WorkflowStep {
            id: "evaluate_files".to_string(),
            name: "File Evaluation".to_string(),
            description: format!(
                "Analyze and evaluate files created by previous agents. Consider code quality, correctness, completeness, and adherence to best practices. Files available for evaluation: {:?}",
                files_list
            ),
            execution_mode: StepExecutionMode::OneShot, // Evaluator doesn't need tools, just analyzes
            required_tools: vec![], // No tools needed for evaluation
            parameters: step_parameters,
        }]
    }
}


