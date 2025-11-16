//! Human-in-the-loop (HITL) system

pub mod assessor;
pub mod queue;
pub mod audit;

pub use assessor::HitlAssessor;
pub use queue::HitlQueue;
pub use audit::HitlAudit;

use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlRequest {
    pub request_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub description: String,
    pub risk_level: RiskLevel,
    pub proposed_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlResponse {
    pub request_id: String,
    pub approved: bool,
    pub feedback: Option<String>,
}
