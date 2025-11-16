//! Agent-Network: Dynamic multi-agent orchestration framework
//!
//! A DAG-based multi-agent orchestration system providing:
//! - Dynamic workflow generation using petgraph
//! - Wave-based parallel execution with dependency resolution
//! - HITL (Human-in-the-loop) integration
//! - Tool integration via MCP
//! - Smart RAG and history context injection
//! - Real-time status streaming

// Module declarations
pub mod error;
pub mod orchestrator;
pub mod workflow;
pub mod agents;
pub mod tools;
pub mod hitl;
pub mod conflict;
pub mod filelocks;
pub mod coordination;
pub mod sharedcontext;
pub mod status_stream;
pub mod token_budget;
pub mod tracing_setup;
pub mod acp;

// Public re-exports for convenience
pub use error::{AgentNetworkError};
pub use orchestrator::Orchestrator;
pub use workflow::{WorkflowBuilder, WorkflowExecutor, WorkflowGraph, TaskNode, TaskResult};
pub use agents::{Agent, AgentType, AgentResult};
pub use status_stream::{StatusEvent, StatusEventType};

// Version constant
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the agent-network library with defaults
pub async fn initialize() -> anyhow::Result<()> {
    tracing::info!("Initializing agent-network v{}", VERSION);
    Ok(())
}
