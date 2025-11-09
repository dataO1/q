use ai_agent_common::*;
use tokio::sync::mpsc;

use crate::hitl::assessor::RiskLevel;

pub struct ApprovalQueue {
    tx: mpsc::Sender<ApprovalRequest>,
    rx: mpsc::Receiver<ApprovalResponse>,
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub task_id: TaskId,
    pub description: String,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone)]
pub struct ApprovalResponse {
    pub task_id: TaskId,
    pub approved: bool,
}

impl ApprovalQueue {
    pub fn new() -> Self {
        todo!("Initialize approval queue")
    }

    pub async fn request_approval(&self, request: ApprovalRequest) -> Result<bool> {
        todo!("Block until user approves/rejects")
    }
}
