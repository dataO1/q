//! Coordination logic for agents and tasks

use std::collections::HashMap;
use std::sync::Arc;
use crate::error::AgentResult;
use tokio::sync::RwLock;

pub struct CoordinationManager {
    task_states: Arc<RwLock<HashMap<String, TaskState>>>,
}

#[derive(Debug, Clone)]
pub struct TaskState {
    pub task_id: String,
    pub agent_id: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl CoordinationManager {
    pub fn new() -> Self {
        Self {
            task_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register new task
    pub async fn register_task(&self, task_id: String, agent_id: String) -> AgentResult<()> {
        let mut states = self.task_states.write().await;
        states.insert(task_id.clone(), TaskState {
            task_id,
            agent_id,
            status: TaskStatus::Pending,
        });
        Ok(())
    }

    /// Update task status
    pub async fn update_task_status(&self, task_id: &str, status: TaskStatus) -> AgentResult<()> {
        let mut states = self.task_states.write().await;

        if let Some(state) = states.get_mut(task_id) {
            state.status = status;
        }

        Ok(())
    }

    /// Get task state
    pub async fn get_task_state(&self, task_id: &str) -> Option<TaskState> {
        let states = self.task_states.read().await;
        states.get(task_id).cloned()
    }

    /// Get all task states
    pub async fn get_all_states(&self) -> HashMap<String, TaskState> {
        let states = self.task_states.read().await;
        states.clone()
    }
}

impl Default for CoordinationManager {
    fn default() -> Self {
        Self::new()
    }
}
