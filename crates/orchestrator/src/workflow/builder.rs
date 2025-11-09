use ai_agent_common::*;
use petgraph::graph::DiGraph;

pub struct WorkflowBuilder {
    graph: DiGraph<SubTask, Dependency>,
}

impl WorkflowBuilder {
    pub fn new() -> Self {
        todo!("Initialize graph")
    }

    pub async fn build_from_decomposition(
        &mut self,
        task: &str,
        subtasks: Vec<SubTask>,
        dependencies: Vec<(TaskId, TaskId)>,
    ) -> Result<()> {
        todo!("Build DAG from subtasks")
    }
}

#[derive(Debug, Clone)]
pub struct SubTask {
    pub id: TaskId,
    pub description: String,
    pub agent_type: AgentType,
    pub target_files: Vec<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Dependency;
