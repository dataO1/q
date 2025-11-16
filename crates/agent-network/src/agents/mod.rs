//! Agent abstractions and implementations

pub mod base;
pub mod coding;
pub mod planning;
pub mod writing;
pub mod evaluator;
pub mod pool;

pub use base::{Agent, AgentType, AgentContext};
pub use pool::AgentPool;

/// Agent execution result
#[derive(Debug, Clone)]
pub struct AgentResult {
    pub agent_id: String,
    pub output: String,
    pub confidence: f32,
    pub requires_hitl: bool,
}
