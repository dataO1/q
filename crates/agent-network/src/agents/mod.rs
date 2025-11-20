//! Agent implementations and management
//!
//! Provides the agent abstractions and concrete implementations.

pub mod base;
pub mod coding;
pub mod evaluator;
pub mod planning;
pub mod pool;
pub mod writing;

pub use base::{Agent, AgentContext, ToolResult, ConversationMessage};
pub use coding::CodingAgent;
pub use evaluator::EvaluatorAgent;
use ollama_rs::generation::chat::ChatMessageResponse;
pub use planning::{PlanningAgent};
pub use pool::{AgentPool, PoolStatistics};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use anyhow::{anyhow, Context, Result};
pub use writing::WritingAgent;

/// Result from agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Agent that produced this result
    pub agent_id: String,

    /// The actual output/response
    pub output: Value,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Whether human review is needed
    pub requires_hitl: bool,

    /// Tokens used in execution
    pub tokens_used: Option<usize>,

    /// Reasoning or explanation
    pub reasoning: Option<String>,
}

impl AgentResult {
    /// Create new agent result
    pub fn from_response(agent_id: &str, response: ChatMessageResponse) -> anyhow::Result<Self> {
        let output = serde_json::from_str(&response.message.content)?;
        Ok(Self {
            agent_id: agent_id.to_string(),
            output,
            confidence: 0.8,
            requires_hitl: false,
            tokens_used: None,
            reasoning: None,
        })
    }

    /// Create new agent result
    pub fn from_string(agent_id: &str, input: &str) -> anyhow::Result<Self> {
        let output: Value = serde_json::from_str(input)?;
        Ok(Self {
            agent_id: agent_id.to_string(),
            output,
            confidence: 0.8,
            requires_hitl: false,
            tokens_used: None,
            reasoning: None,
        })
    }

    /// Extract typed output from the JSON Value
    ///
    /// This deserializes the stored JSON output into any type T
    /// that implements Deserialize
    pub fn extract<T>(&self) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_value(self.output.clone())
            .context("Failed to deserialize agent output into requested type")
    }

    /// Set confidence score
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Mark as requiring HITL
    pub fn requiring_hitl(mut self) -> Self {
        self.requires_hitl = true;
        self
    }

    /// Set tokens used
    pub fn with_tokens(mut self, tokens: usize) -> Self {
        self.tokens_used = Some(tokens);
        self
    }

    /// Set reasoning
    pub fn with_reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = Some(reasoning);
        self
    }
}
