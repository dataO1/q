//! Workflow DAG handling and execution

pub mod builder;
pub mod executor;
pub mod analyzer;

use ai_agent_common::ErrorRecoveryStrategy;
pub use builder::WorkflowBuilder;
pub use executor::WorkflowExecutor;
pub use analyzer::WorkflowAnalyzer;

use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};

/// Workflow graph type
pub type WorkflowGraph = DiGraph<TaskNode, DependencyEdge>;

/// Task node in the workflow graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    pub task_id: String,
    pub agent_id: String,
    pub description: String,
    pub recovery_strategy: ErrorRecoveryStrategy,
    pub requires_hitl: bool,
}

/// Dependency edge between tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    Sequential,
    Conditional,
}

/// Result of a task execution
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}
