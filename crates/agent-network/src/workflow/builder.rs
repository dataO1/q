//! Workflow graph builder using petgraph

use crate::{
    error::AgentNetworkError, workflow::{DependencyEdge, DependencyType, TaskNode, WorkflowGraph}
};
use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use anyhow::Result;

pub struct WorkflowBuilder {
    graph: WorkflowGraph,
    task_indices: HashMap<String, NodeIndex>,
}

impl WorkflowBuilder {
    pub fn new() -> Self {
        Self {
            graph: WorkflowGraph::new(),
            task_indices: HashMap::new(),
        }
    }

    /// Add a task node to the workflow
    pub fn add_task(&mut self, task: TaskNode) -> Result<NodeIndex> {
        let task_id = task.task_id.clone();
        let index = self.graph.add_node(task);
        self.task_indices.insert(task_id, index);
        Ok(index)
    }

    /// Add a dependency edge between tasks
    pub fn add_dependency(
        &mut self,
        from_task_id: &str,
        to_task_id: &str,
        dependency_type: DependencyType,
    ) -> Result<()> {
        let from_idx = self.task_indices.get(from_task_id)
            .ok_or_else(|| AgentNetworkError::Workflow(
                format!("Task not found: {}", from_task_id)
            ))?;
        let to_idx = self.task_indices.get(to_task_id)
            .ok_or_else(|| AgentNetworkError::Workflow(
                format!("Task not found: {}", to_task_id)
            ))?;

        self.graph.add_edge(*from_idx, *to_idx, DependencyEdge { dependency_type });
        Ok(())
    }

    /// Build and return the workflow graph
    pub fn build(self) -> WorkflowGraph {
        self.graph
    }

    /// Get task index by ID
    pub fn get_task_index(&self, task_id: &str) -> Option<NodeIndex> {
        self.task_indices.get(task_id).copied()
    }
}

impl Default for WorkflowBuilder {
    fn default() -> Self {
        Self::new()
    }
}
