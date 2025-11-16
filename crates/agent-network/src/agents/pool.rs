//! Agent pool management

use crate::agents::evaluator::EvaluatorAgent;
use crate::agents::planning::PlanningAgent;
use crate::agents::writing::WritingAgent;
use crate::agents::{coding::CodingAgent, Agent, AgentType};
use std::collections::HashMap;
use std::sync::Arc;
use ai_agent_common::{AgentConfig, QualityStrategy};
use crate::error::{AgentNetworkError, AgentNetworkResult};

pub struct AgentPool {
    agents: HashMap<String, Arc<dyn Agent>>,
}

impl AgentPool {
    pub async fn new(configs: &Vec<AgentConfig>) -> AgentNetworkResult<Self> {
        let mut agents: HashMap<String, Arc<dyn Agent>> = HashMap::new();

        for config in configs {
            let agent: Arc<dyn Agent> = match config.agent_type.as_str() {
                "coding" => Arc::new(CodingAgent::new(
                    config.id.clone(),
                    config.model.clone(),
                )),
                "planning" => Arc::new(PlanningAgent::new(
                    config.id.clone(),
                    config.model.clone(),
                )),
                "writing" => Arc::new(WritingAgent::new(
                    config.id.clone(),
                    config.model.clone(),
                )),
                "evaluator" => Arc::new(EvaluatorAgent::new(
                    config.id.clone(),
                    config.model.clone(),
                    QualityStrategy::Always,
                )),
                _ => return Err(AgentNetworkError::Config(
                    format!("Unknown agent type: {}", config.agent_type)
                )),
            };

            agents.insert(config.id.clone(), agent);
        }

        Ok(Self { agents })
    }

    pub fn get_agent(&self, agent_id: &str) -> Option<Arc<dyn Agent>> {
        self.agents.get(agent_id).cloned()
    }

    pub fn get_agents_by_type(&self, agent_type: AgentType) -> Vec<Arc<dyn Agent>> {
        self.agents
            .values()
            .filter(|agent| {
                matches!(
                    (agent.agent_type(), agent_type),
                    (AgentType::Coding, AgentType::Coding)
                    | (AgentType::Planning, AgentType::Planning)
                    | (AgentType::Writing, AgentType::Writing)
                    | (AgentType::Evaluator, AgentType::Evaluator)
                )
            })
            .cloned()
            .collect()
    }
}
