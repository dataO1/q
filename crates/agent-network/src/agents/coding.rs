//! Coding agent implementation using Rig LLM framework
//!
//! The coding agent specializes in generating, reviewing, and refactoring code.
//! It integrates with Rig for LLM calls and supports local Ollama models.

use crate::{ agents::{base::TypedAgent, Agent, AgentContext, AgentResult}, error::AgentNetworkResult};
use ai_agent_common::AgentType;
use async_trait::async_trait;
use ollama_rs::Ollama;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CodingOutput {
    pub code: String,
    pub language: String,
    pub explanation: String,
    pub tests: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
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
        Self {
            id,
            client,
            system_prompt,
            model,
            temperature: temperature.clamp(0.0, 2.0),
            max_tokens,
        }
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

    fn build_prompt(&self, context: &AgentContext) -> String {
        let mut parts = vec![format!("# Coding Task: {}", context.description)];

        if let Some(ref rag) = context.rag_context {
            parts.push(format!("\n## Code Context:\n{}", rag));
        }

        if let Some(ref hist) = context.history_context {
            parts.push(format!("\n## History:\n{}", hist));
        }

        // if !context.dependency_outputs.is_empty() {
        //     parts.push("\n## Previous Outputs:".to_string());
        //     for (id, out) in &context.dependency_outputs {
        //         parts.push(format!("- {}: {}", id, out));
        //     }
        // }

        parts.push("\n## Instructions:".to_string());
        parts.push("Generate production-ready code with explanations.".to_string());

        parts.join("\n")
    }
}
