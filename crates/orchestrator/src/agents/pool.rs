use ai_agent_common::*;
use rig::agent::Agent;
use rig::completion::CompletionModel;
use std::sync::Arc;
use reqwest::Client;

use crate::OllamaModel;

pub struct AgentPool {
    orchestrator: Arc<Agent<OllamaModel>>,
    coding_agents: Vec<Arc<Agent<OllamaModel>>>,
    planning_agents: Vec<Arc<Agent<OllamaModel>>>,
    writing_agents: Vec<Arc<Agent<OllamaModel>>>,
}

impl AgentPool {
    pub async fn new(configs: &[AgentConfig]) -> Result<Self> {
        todo!("Initialize all agents from config")
    }

    pub fn get_agent(&self, agent_type: AgentType) -> Result<Arc<Agent<OllamaModel>>> {
        todo!("Get available agent of type")
    }
}
