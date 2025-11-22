//! Coding agent implementation using Rig LLM framework
//!
//! The coding agent specializes in generating, reviewing, and refactoring code.
//! It integrates with Rig for LLM calls and supports local Ollama models.

use crate::agents::base::TypedAgent;
use ai_agent_common::AgentType;
use async_trait::async_trait;
use ollama_rs::Ollama;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CodingOutput {
    // pub code: String,
    // pub language: String,
    pub change_log: Vec<ChangeLog>,
    // pub tests: Option<String>,
    // #[serde(default)]
    // pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChangeLog {
    changed_file: String,
    summary_of_changes: String,
}


/// Coding agent for code generation and review
pub struct CodingAgent {
    id: String,
    model: String,
    client: Ollama,
    system_prompt: String,
    temperature: f32,
    max_tokens: usize,
}

impl CodingAgent {
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
        let system_prompt = CodingAgent::build_system_prompt(system_prompt);
        Self {
            id,
            client,
            system_prompt,
            model,
            temperature: temperature.clamp(0.0, 2.0),
            max_tokens,
        }
    }

    fn build_system_prompt(prompt: String) -> String{
        let tools_usage = r#"
            ## CRITICAL TOOL-USAGE RULES:
            - You can use the "list", "exists", "read" and "write" functions of the filesystem tool as described
            - Do NOT use "delete" or "metadata" functions
            - You MUST use the filesystem tool to write all generated code to files
            - DO NOT return code in your message - always write it using tools
            - DO NOT write the change_log as a file output, just as a resulting message

            ## YOUR WORKFLOW:
            1. Analyze which code files are relevant for your coding task, given your information (RAG Context, History, Available Tools and user prompts).
            2. If youre unsure, check if code files already exist and if so then read them.
            3. Based on the current status of existing code, mentally generate the new code file based on the prompts by the user.
            4. Call filesystem tool to write the generated code to the corresponding files.
            5. Output a detailed change_log of the changes you made to any file.

            NEVER output code directly in your response."
            }"#;
        format!("##{}\n{}", prompt, tools_usage)
    }
}

#[async_trait]
impl TypedAgent for CodingAgent {
    fn id(&self) -> &str { &self.id }
    fn agent_type(&self) -> AgentType { AgentType::Coding }
    fn system_prompt(&self) -> &str { &self.system_prompt }
    fn model(&self) -> &str { &self.model }
    fn temperature(&self) -> f32 { self.temperature }
    fn client(&self) -> &Ollama { &self.client }
    type Output = CodingOutput;
    
    /// Define coding workflow steps
    fn define_workflow_steps(&self, context: &crate::agents::AgentContext) -> Vec<crate::agents::base::WorkflowStep> {
        use crate::agents::base::{WorkflowStep, StepExecutionMode};
        use std::collections::HashMap;
        
        // Multi-step workflow for coding: analysis, implementation, validation
        vec![
            WorkflowStep {
                id: "analyze_codebase".to_string(),
                name: "Codebase Analysis".to_string(),
                description: "Analyze relevant code files, understand existing structure and dependencies for the coding task".to_string(),
                execution_mode: StepExecutionMode::ReAct { max_iterations: Some(10) }, // Needs filesystem tool for reading
                required_tools: vec!["filesystem".to_string()],
                parameters: HashMap::new(),
            },
            WorkflowStep {
                id: "implement_code".to_string(),
                name: "Code Implementation".to_string(),
                description: "Generate and write the actual code based on requirements and existing codebase analysis".to_string(),
                execution_mode: StepExecutionMode::ReAct { max_iterations: Some(15) }, // Needs filesystem tool for writing
                required_tools: vec!["filesystem".to_string()],
                parameters: HashMap::new(),
            }
        ]
    }
}
