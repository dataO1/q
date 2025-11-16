//! Agent implementations and management
//!
//! Provides the agent abstractions and concrete implementations.

pub mod base;
pub mod coding;
pub mod evaluator;
pub mod planning;
pub mod pool;
pub mod writing;

pub use base::{Agent, AgentContext,  AgentType, ToolResult, ConversationMessage};
pub use coding::CodingAgent;
pub use evaluator::EvaluatorAgent;
pub use planning::{PlanningAgent, PlanStep};
pub use pool::{AgentPool, PoolStatistics};
pub use writing::WritingAgent;

/// Result from agent execution
#[derive(Debug, Clone)]
pub struct AgentResult {
    /// Agent that produced this result
    pub agent_id: String,

    /// The actual output/response
    pub output: String,

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
    pub fn new(agent_id: String, output: String) -> Self {
        Self {
            agent_id,
            output,
            confidence: 0.8,
            requires_hitl: false,
            tokens_used: None,
            reasoning: None,
        }
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
