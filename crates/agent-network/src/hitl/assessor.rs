//! HITL risk assessor

use ai_agent_common::HitlMode;

use crate::{
    hitl::{HitlRequest, RiskLevel},
    error::Result,
};

pub struct HitlAssessor {
    mode: HitlMode,
}

impl HitlAssessor {
    pub fn new(mode: HitlMode) -> Self {
        Self { mode }
    }

    /// Assess risk level for a task
    pub async fn assess_risk(&self, task_description: &str) -> Result<RiskLevel> {
        // TODO: Week 5 - Implement risk assessment logic
        // - Analyze task description
        // - Determine risk level
        // - Apply heuristics

        Ok(RiskLevel::Low)
    }

    /// Determine if HITL is required based on mode and risk
    pub fn requires_hitl(&self, risk_level: &RiskLevel) -> bool {
        match (&self.mode, risk_level) {
            (HitlMode::Blocking, _) => true,
            (HitlMode::Async, RiskLevel::High | RiskLevel::Critical) => true,
            (HitlMode::SampleBased, RiskLevel::Critical) => true,
            _ => false,
        }
    }
}
