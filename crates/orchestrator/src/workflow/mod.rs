pub mod builder;
pub mod executor;
pub mod analyzer;
pub mod checkpoint;

use ai_agent_common::*;
use rig::completion::CompletionModel;
use crate::agents::AgentPool;
use petgraph::graph::DiGraph;

/// Main workflow engine managing task graph execution
pub struct WorkflowEngine {
    graph: DiGraph<builder::SubTask, builder::Dependency>,
    executor: executor::WaveExecutor,
    analyzer: analyzer::ComplexityAnalyzer,
    checkpoint_manager: checkpoint::CheckpointManager,
}

impl WorkflowEngine {
    pub async fn new(database_url: &str) -> Result<Self> {
        Ok(Self {
            graph: DiGraph::new(),
            executor: executor::WaveExecutor::new(),
            analyzer: analyzer::ComplexityAnalyzer::new(),
            checkpoint_manager: checkpoint::CheckpointManager::new(database_url).await?,
        })
    }

    pub async fn execute<M:CompletionModel>(&mut self, agents: &AgentPool<M>) -> Result<String> {
        todo!("Execute workflow graph with wave-based execution")
    }

    pub async fn save_checkpoint(&self) -> Result<uuid::Uuid> {
        todo!("Save current workflow state")
    }
}
