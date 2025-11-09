
//! Audit logging for HITL approval decisions

use std::sync::Mutex;
use std::collections::HashMap;
use ai_agent_common::*;
use chrono::{DateTime, Utc};

pub struct AuditLog {
    pub log: Mutex<HashMap<TaskId, Vec<AuditEntry>>>,
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub approved: bool,
    pub user_id: Option<String>,
    pub comment: Option<String>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self {
            log: Mutex::new(HashMap::new()),
        }
    }

    pub fn record(&self, task_id: TaskId, approved: bool, user_id: Option<String>, comment: Option<String>) {
        let mut log = self.log.lock().unwrap();
        let entry = AuditEntry {
            timestamp: Utc::now(),
            approved,
            user_id,
            comment,
        };
        log.entry(task_id).or_default().push(entry);
    }

    pub fn entries(&self, task_id: TaskId) -> Vec<AuditEntry> {
        self.log.lock().unwrap()
            .get(&task_id)
            .cloned()
            .unwrap_or_default()
    }
}
