//! Agent pool for managing and retrieving agents
//!
//! Centralized pool that instantiates and caches all agents.
//! Provides agent lookup by ID and type.

use crate::agents::{
    coding::CodingAgent, evaluator::EvaluatorAgent, planning::PlanningAgent,
    writing::WritingAgent, Agent,
};
use crate::error::{AgentNetworkError, AgentNetworkResult};
use std::collections::HashMap;
use std::sync::Arc;
use ai_agent_common::{AgentConfig, AgentType, QualityStrategy, SystemConfig};
use tracing::{debug, info, instrument};

/// Agent pool managing all available agents
pub struct AgentPool {
    /// Map of agent ID -> Agent trait object
    agents: HashMap<String, Arc<dyn Agent>>,

    /// Index of agents by type for quick lookup
    agents_by_type: HashMap<AgentType, Vec<String>>,
}

impl AgentPool {
    /// Create a new agent pool from configurations
    pub async fn new(config: &SystemConfig) -> AgentNetworkResult<Self> {
        let mut agents: HashMap<String, Arc<dyn Agent>> = HashMap::new();
        let mut agents_by_type: HashMap<AgentType, Vec<String>> = HashMap::new();
        let ollama_url = config.embedding.ollama_host.clone() +":" + &config.embedding.ollama_port.to_string() + "/v1";

        for config in &config.agent_network.agents {
            debug!("Initializing agent: {} ({})", config.id, config.agent_type);

            let agent: Arc<dyn Agent> = match config.agent_type {
                AgentType::Coding => Arc::new(CodingAgent::new(
                    config.id.clone(),
                    config.model.clone(),
                    config.system_prompt.clone(),
                    config.temperature,
                    config.max_tokens,
                    Some(&ollama_url),
                )),
                AgentType::Planning => Arc::new(PlanningAgent::new(
                    config.id.clone(),
                    config.model.clone(),
                    config.system_prompt.clone(),
                    config.temperature,
                    config.max_tokens,
                    Some(&ollama_url),
                )),
                AgentType::Evaluator => {
                    let quality_strategy = config
                        .quality_strategy
                        .unwrap_or(QualityStrategy::OnlyForCritical);

                    Arc::new(EvaluatorAgent::new(
                        config.id.clone(),
                        config.model.clone(),
                        config.system_prompt.clone(),
                        config.temperature,
                        config.max_tokens,
                        quality_strategy,
                        Some(&ollama_url),
                    ))
                }
                _ => {
                    return Err(AgentNetworkError::config(format!(
                        "Unknown agent type: {}",
                        config.agent_type
                    )))
                }
            };

            agents.insert(config.id.clone(), agent);
            agents_by_type
                .entry(config.agent_type.clone())
                .or_insert_with(Vec::new)
                .push(config.id.clone());
        }

        if agents.is_empty() {
            return Err(AgentNetworkError::config(
                "No agents configured in pool",
            ));
        }

        info!("Agent pool initialized with {} agents", agents.len());

        Ok(Self {
            agents,
            agents_by_type,
        })
    }

    /// Get agent by ID
    pub fn get_agent(&self, agent_id: &str) -> Option<Arc<dyn Agent>> {
        self.agents.get(agent_id).cloned()
    }

    /// Get all agents of a specific type
    pub fn get_agents_by_type(&self, agent_type: AgentType) -> Vec<Arc<dyn Agent>> {
        self.agents_by_type
            .get(&agent_type)
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|id| self.agents.get(id).cloned())
            .collect()
    }

    /// Get first agent of a specific type
    pub fn get_agent_by_type(&self, agent_type: AgentType) -> Option<Arc<dyn Agent>> {
        self.get_agents_by_type(agent_type).into_iter().next()
    }

    /// Get all agents
    pub fn get_all_agents(&self) -> Vec<Arc<dyn Agent>> {
        self.agents.values().cloned().collect()
    }

    /// Get agent count
    pub fn count(&self) -> usize {
        self.agents.len()
    }

    /// List all agent IDs
    pub fn list_agent_ids(&self) -> Vec<String> {
        self.agents.keys().cloned().collect()
    }

    /// Check if agent exists
    pub fn has_agent(&self, agent_id: &str) -> bool {
        self.agents.contains_key(agent_id)
    }

    /// Get statistics about the pool
    pub fn statistics(&self) -> PoolStatistics {
        let mut stats_by_type: HashMap<AgentType, usize> = HashMap::new();

        for (type_str, ids) in &self.agents_by_type {
            stats_by_type.insert(type_str.clone(), ids.len());
        }

        PoolStatistics {
            total_agents: self.agents.len(),
            agents_by_type: stats_by_type,
        }
    }
}

/// Statistics about agent pool
#[derive(Debug, Clone)]
pub struct PoolStatistics {
    pub total_agents: usize,
    pub agents_by_type: HashMap<AgentType, usize>,
}
