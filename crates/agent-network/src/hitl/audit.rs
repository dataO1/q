//! HITL audit log

use crate::{
    hitl::{HitlRequest, HitlResponse},
    error::Result,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub request: HitlRequest,
    pub response: Option<HitlResponse>,
}

pub struct HitlAudit {
    entries: Vec<AuditEntry>,
}

impl HitlAudit {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Log HITL request
    pub async fn log_request(&mut self, request: HitlRequest) -> Result<()> {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            request,
            response: None,
        };
        self.entries.push(entry);
        Ok(())
    }

    /// Log HITL response
    pub async fn log_response(&mut self, request_id: &str, response: HitlResponse) -> Result<()> {
        // TODO: Week 5 - Update corresponding request with response
        if let Some(entry) = self.entries.iter_mut().find(|e| e.request.request_id == request_id) {
            entry.response = Some(response);
        }
        Ok(())
    }

    /// Get all audit entries
    pub fn get_entries(&self) -> &[AuditEntry] {
        &self.entries
    }
}

impl Default for HitlAudit {
    fn default() -> Self {
        Self::new()
    }
}
