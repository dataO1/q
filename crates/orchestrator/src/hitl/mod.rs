pub mod queue;
pub mod assessor;
pub mod audit;

use ai_agent_common::*;

/// Human-In-The-Loop orchestrator managing approval workflows
pub struct HitlOrchestrator {
    approval_queue: queue::ApprovalQueue,
    risk_assessor: assessor::RiskAssessor,
    audit_log: audit::AuditLog,
}

impl HitlOrchestrator {
    pub fn new() -> Self {
        Self {
            approval_queue: queue::ApprovalQueue::new(),
            risk_assessor: assessor::RiskAssessor::new(),
            audit_log: audit::AuditLog::new(),
        }
    }

    pub async fn request_approval(
        &self,
        task_id: TaskId,
        description: String,
    ) -> Result<bool> {
        // Assess risk
        let risk_level = self.risk_assessor.assess_risk(&description);

        // Create approval request
        let request = queue::ApprovalRequest {
            task_id,
            description,
            risk_level,
        };

        // Wait for approval
        let approved = self.approval_queue.request_approval(request).await?;

        // Log decision
        self.audit_log.record(task_id, approved, None, None);

        Ok(approved)
    }
}
