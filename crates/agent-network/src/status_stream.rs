//! Real-time status streaming via channels

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEvent {
    pub event_type: StatusEventType,
    pub agent_id: Option<String>,
    pub task_id: Option<String>,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatusEventType {
    OrchestratorStarted,
    OrchestratorCompleted,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    AgentStarted,
    AgentCompleted,
    AgentFailed,
    ToolExecuted,
    HitlRequested,
    HitlCompleted,
}

pub struct StatusStream {
    sender: broadcast::Sender<StatusEvent>,
}

impl StatusStream {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }

    /// Emit status event
    pub fn emit(&self, event: StatusEvent) {
        // Ignore send errors (no receivers)
        let _ = self.sender.send(event);
    }

    /// Subscribe to status events
    pub fn subscribe(&self) -> broadcast::Receiver<StatusEvent> {
        self.sender.subscribe()
    }

    /// Helper to emit task started event
    pub fn emit_task_started(&self, task_id: String, agent_id: String, message: String) {
        self.emit(StatusEvent {
            event_type: StatusEventType::TaskStarted,
            agent_id: Some(agent_id),
            task_id: Some(task_id),
            message,
            timestamp: Utc::now(),
        });
    }

    /// Helper to emit task completed event
    pub fn emit_task_completed(&self, task_id: String, agent_id: String, message: String) {
        self.emit(StatusEvent {
            event_type: StatusEventType::TaskCompleted,
            agent_id: Some(agent_id),
            task_id: Some(task_id),
            message,
            timestamp: Utc::now(),
        });
    }

    /// Helper to emit task failed event
    pub fn emit_task_failed(&self, task_id: String, agent_id: String, message: String) {
        self.emit(StatusEvent {
            event_type: StatusEventType::TaskFailed,
            agent_id: Some(agent_id),
            task_id: Some(task_id),
            message,
            timestamp: Utc::now(),
        });
    }
}

impl Default for StatusStream {
    fn default() -> Self {
        Self::new()
    }
}
