//! Agent-Network: Dynamic multi-agent orchestration framework
//!
//! This crate provides a DAG-based multi-agent orchestration system with:
//! - Dynamic workflow generation using petgraph
//! - Wave-based parallel execution with dependency resolution
//! - HITL (Human-in-the-loop) integration
//! - Tool integration via MCP
//! - Smart RAG and history context injection
//! - Real-time status streaming

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

// Re-exports
pub use orchestrator::Orchestrator;
pub use workflow::{WorkflowBuilder, WorkflowExecutor};
pub use agents::{Agent, AgentType};
pub use status_stream::StatusEvent;

use ai_agent_common::*;

pub type OllamaModel = rig::providers::ollama::CompletionModel<reqwest::Client>;
