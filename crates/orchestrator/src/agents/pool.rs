use ai_agent_common::*;
use std::sync::Arc;

pub struct AgentPool {
    orchestrator: Arc<rig_core::Agent>,
    coding_agents: Vec<Arc<rig_core::Agent>>,
    planning_agents: Vec<Arc<rig_core::Agent>>,
    writing_agents: Vec<Arc<rig_core::Agent>>,
}

impl AgentPool {
    pub async fn new(configs: &[AgentConfig]) -> Result<Self> {
        todo!("Initialize all agents from config")
    }

    pub fn get_agent(&self, agent_type: AgentType) -> Result<Arc<rig_core::Agent>> {
        todo!("Get available agent of type")
    }
}
