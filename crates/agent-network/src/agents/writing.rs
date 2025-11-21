//! Writing agent for documentation and communication
//!
//! Generates documentation, commit messages, and communication.

use crate::{agents::{base::TypedAgent, Agent, AgentContext, AgentResult}};
use ai_agent_common::AgentType;
use async_trait::async_trait;
use ollama_rs::Ollama;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Writing task structured output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WritingOutput {
    pub content: String,
    pub format: String,
    pub word_count: usize,
    #[serde(default)]
    pub topics: Vec<String>,
}

pub struct WritingAgent {
    id: String,
    model: String,
    system_prompt: String,
    temperature: f32,
    max_tokens: usize,
    client: Ollama,
}

impl WritingAgent {
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
impl TypedAgent for WritingAgent {
    fn id(&self) -> &str { &self.id }
    fn agent_type(&self) -> AgentType { AgentType::Writing }
    fn system_prompt(&self) -> &str { &self.system_prompt }
    fn model(&self) -> &str { &self.model }
    fn temperature(&self) -> f32 { self.temperature }
    fn client(&self) -> &Ollama { &self.client }
    type Output = WritingOutput;

}
