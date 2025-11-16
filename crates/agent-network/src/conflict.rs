//! Conflict resolution for concurrent agent operations

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::AgentResult;

pub struct ConflictResolver {
    active_operations: RwLock<HashMap<String, Vec<String>>>,
}

impl ConflictResolver {
    pub fn new() -> Self {
        Self {
            active_operations: RwLock::new(HashMap::new()),
        }
    }

    /// Check if operation conflicts with active operations
    pub async fn check_conflict(&self, resource: &str, agent_id: &str) -> AgentResult<bool> {
        let operations = self.active_operations.read().await;

        if let Some(agents) = operations.get(resource) {
            Ok(!agents.is_empty() && !agents.contains(&agent_id.to_string()))
        } else {
            Ok(false)
        }
    }

    /// Register operation on resource
    pub async fn register_operation(&self, resource: String, agent_id: String) -> AgentResult<()> {
        let mut operations = self.active_operations.write().await;
        operations
            .entry(resource)
            .or_insert_with(Vec::new)
            .push(agent_id);
        Ok(())
    }

    /// Unregister operation on resource
    pub async fn unregister_operation(&self, resource: &str, agent_id: &str) -> AgentResult<()> {
        let mut operations = self.active_operations.write().await;

        if let Some(agents) = operations.get_mut(resource) {
            agents.retain(|id| id != agent_id);
            if agents.is_empty() {
                operations.remove(resource);
            }
        }

        Ok(())
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new()
    }
}
