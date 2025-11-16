//! Task coordination and state management
//!
//! Manages task lifecycle, dependencies, and state transitions
//! across concurrent agent execution.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::error::AgentNetworkResult;

/// Task execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Running => write!(f, "Running"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Skipped => write!(f, "Skipped"),
        }
    }
}

/// Task state information
#[derive(Debug, Clone)]
pub struct TaskState {
    /// Unique task identifier
    pub task_id: String,

    /// Agent executing the task
    pub agent_id: String,

    /// Current status
    pub status: TaskStatus,

    /// Number of retries so far
    pub retry_count: u32,

    /// Maximum retries allowed
    pub max_retries: u32,

    /// Metadata for tracking
    pub metadata: HashMap<String, String>,
}

/// Coordination manager for multi-agent workflows
pub struct CoordinationManager {
    /// Task states
    states: Arc<RwLock<HashMap<String, TaskState>>>,

    /// Task dependency graph
    dependencies: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl CoordinationManager {
    /// Create a new coordination manager
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            dependencies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new task
    pub async fn register_task(&self, task_id: String, agent_id: String) -> AgentNetworkResult<()> {
        let mut states = self.states.write().await;

        states.insert(
            task_id.clone(),
            TaskState {
                task_id,
                agent_id,
                status: TaskStatus::Pending,
                retry_count: 0,
                max_retries: 3,
                metadata: HashMap::new(),
            },
        );

        Ok(())
    }

    /// Update task status
    pub async fn update_task_status(&self, task_id: &str, status: TaskStatus) -> AgentNetworkResult<()> {
        let mut states = self.states.write().await;

        if let Some(state) = states.get_mut(task_id) {
            state.status = status;
            debug!("Task {} status updated to {}", task_id, status);
        }

        Ok(())
    }

    /// Increment retry count
    pub async fn increment_retry_count(&self, task_id: &str) -> AgentNetworkResult<u32> {
        let mut states = self.states.write().await;

        if let Some(state) = states.get_mut(task_id) {
            state.retry_count += 1;
            Ok(state.retry_count)
        } else {
            Err(crate::error::AgentNetworkError::NotFound {
                resource_type: "task".to_string(),
                resource_id: task_id.to_string(),
            })
        }
    }

    /// Get task state
    pub async fn get_task_state(&self, task_id: &str) -> Option<TaskState> {
        let states = self.states.read().await;
        states.get(task_id).cloned()
    }

    /// Get all task states
    pub async fn get_all_states(&self) -> HashMap<String, TaskState> {
        let states = self.states.read().await;
        states.clone()
    }

    /// Register task dependency
    pub async fn register_dependency(&self, task_id: String, depends_on: String) -> AgentNetworkResult<()> {
        let mut deps = self.dependencies.write().await;
        deps.entry(task_id).or_insert_with(Vec::new).push(depends_on);
        Ok(())
    }

    /// Check if all dependencies are satisfied
    pub async fn are_dependencies_satisfied(&self, task_id: &str) -> AgentNetworkResult<bool> {
        let deps = self.dependencies.read().await;
        let states = self.states.read().await;

        if let Some(task_deps) = deps.get(task_id) {
            for dep_id in task_deps {
                if let Some(state) = states.get(dep_id) {
                    if state.status != TaskStatus::Completed {
                        return Ok(false);
                    }
                } else {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Set metadata for a task
    pub async fn set_metadata(&self, task_id: &str, key: String, value: String) -> AgentNetworkResult<()> {
        let mut states = self.states.write().await;

        if let Some(state) = states.get_mut(task_id) {
            state.metadata.insert(key, value);
            Ok(())
        } else {
            Err(crate::error::AgentNetworkError::NotFound {
                resource_type: "task".to_string(),
                resource_id: task_id.to_string(),
            })
        }
    }

    /// Get task statistics
    pub async fn get_statistics(&self) -> TaskStatistics {
        let states = self.states.read().await;

        let mut stats = TaskStatistics::default();
        stats.total_tasks = states.len();

        for state in states.values() {
            match state.status {
                TaskStatus::Pending => stats.pending_count += 1,
                TaskStatus::Running => stats.running_count += 1,
                TaskStatus::Completed => stats.completed_count += 1,
                TaskStatus::Failed => stats.failed_count += 1,
                TaskStatus::Skipped => stats.skipped_count += 1,
            }

            if state.retry_count > 0 {
                stats.total_retries += state.retry_count as usize;
            }
        }

        stats
    }
}

impl Default for CoordinationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Task statistics
#[derive(Debug, Clone, Default)]
pub struct TaskStatistics {
    pub total_tasks: usize,
    pub pending_count: usize,
    pub running_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub total_retries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_registration() {
        let coord = CoordinationManager::new();
        let result = coord.register_task("task-1".to_string(), "agent-1".to_string()).await;
        assert!(result.is_ok());

        let state = coord.get_task_state("task-1").await;
        assert!(state.is_some());
        assert_eq!(state.unwrap().status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn test_status_update() {
        let coord = CoordinationManager::new();
        coord.register_task("task-1".to_string(), "agent-1".to_string()).await.ok();

        coord.update_task_status("task-1", TaskStatus::Running).await.ok();

        let state = coord.get_task_state("task-1").await;
        assert_eq!(state.unwrap().status, TaskStatus::Running);
    }

    #[test]
    fn test_task_status_display() {
        assert_eq!(TaskStatus::Running.to_string(), "Running");
        assert_eq!(TaskStatus::Completed.to_string(), "Completed");
    }
}
