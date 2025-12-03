//! Human-in-the-loop (HITL) system

//! HITL integration module, pub exports and helper types

pub mod assessor;
pub mod audit;

use std::collections::HashMap;

use ai_agent_common::{AgentType, RiskLevel};
pub use assessor::*;
pub use audit::*;

use serde::{Deserialize, Serialize};
use anyhow::Result;

use crate::AgentResult;


#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub assessment: RiskAssessment,
    pub decision: Option<ApprovalDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ApprovalDecision {
    Approved{ reasoning: Option<String> },
    Rejected{ reasoning: Option<String> },
    NeedsMoreInfo,
}


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
pub struct RiskAssessment {
    pub agent_id: String,
    pub agent_type: AgentType,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub reason: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl RiskAssessment {
    pub fn new(agent_result: &AgentResult, agent_type: AgentType, reason: Option<String>) -> Self {
        let risk_level = RiskLevel::from_confidence(agent_result.confidence);
        Self {
            agent_id: agent_result.agent_id.clone(),
            agent_type,
            confidence: agent_result.confidence,
            risk_level,
            reason,
            metadata: HashMap::new(),
        }
    }

    pub fn needs_hitl(&self, threshold: RiskLevel) -> bool {
        self.risk_level >= threshold
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlResponse {
    pub request_id: String,
    pub approved: bool,
    pub feedback: Option<String>,
}
