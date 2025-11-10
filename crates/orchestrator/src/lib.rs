pub mod agents;
pub mod workflow;
pub mod coordination;
pub mod hitl;

use ai_agent_common::*;

pub type OllamaModel = rig::providers::ollama::CompletionModel<reqwest::Client>;

pub struct OrchestratorSystem {
    agents: agents::AgentPool,
    workflow: workflow::WorkflowEngine,
    coordination: coordination::CoordinationLayer,
    hitl: hitl::HitlOrchestrator,
}

impl OrchestratorSystem {
    pub async fn new(config: &SystemConfig) -> Result<Self> {
        Ok(Self {
            agents: agents::AgentPool::new(&config.orchestrator.agents).await?,
            workflow: workflow::WorkflowEngine::new(&config.storage.postgres_url).await?,
            coordination: coordination::CoordinationLayer::new(),
            hitl: hitl::HitlOrchestrator::new(),
        })
    }

    pub async fn execute_query(
        &mut self,
        query: String,
        conversation_id: ConversationId,
    ) -> Result<String> {
        todo!("Execute full query workflow")
    }

    // Placeholder for status subscription (for API streaming)
    pub async fn subscribe_to_status(&self, task_id: String) -> tokio::sync::broadcast::Receiver<StatusEvent> {
        todo!("Implement status event subscription")
    }

    // Placeholder for agent listing
    pub async fn list_agents(&self) -> Vec<String> {
        todo!("List available agents")
    }
}
