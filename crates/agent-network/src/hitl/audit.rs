//! Structured Audit Logger

use tracing::{info, warn, error};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub agent_id: String,
    pub task_id: String,
    pub action: String,
    pub risk_level: String,
    pub decision: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AuditLogger;

impl AuditLogger {
    pub fn log(event: AuditEvent) {
        info!(target: "audit", "Audit Event: {:?}", event);
    }

    pub fn warn(event: AuditEvent) {
        warn!(target: "audit", "Audit Event: {:?}", event);
    }

    pub fn error(event: AuditEvent) {
        error!(target: "audit", "Audit Event: {:?}", event);
    }
}
