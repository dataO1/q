use ai_agent_common::*;
use rig::agent::Agent;
use rig::completion::CompletionModel;
use std::sync::Arc;

pub struct AgentPool<M: CompletionModel> {
    orchestrator: Arc<Agent<M>>,
    coding_agents: Vec<Arc<Agent<M>>>,
    planning_agents: Vec<Arc<Agent<M>>>,
    writing_agents: Vec<Arc<Agent<M>>>,
}

impl<M:CompletionModel> AgentPool<M> {
    pub async fn new(configs: &[AgentConfig]) -> Result<Self> {
        todo!("Initialize all agents from config")
    }

    pub fn get_agent(&self, agent_type: AgentType) -> Result<Arc<Agent<M>>> {
        todo!("Get available agent of type")
    }
}
