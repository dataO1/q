//! Approval Queue for HITL

use ai_agent_common::HitlMode;
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tracing::{info, warn};

use crate::hitl::{RiskAssessment, RiskLevel};


#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub assessment: RiskAssessment,
    pub decision: Option<ApprovalDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApprovalDecision {
    Approved,
    Rejected,
    NeedsMoreInfo,
}

#[async_trait]
pub trait ApprovalHandler: Send + Sync {
    async fn request_approval(&self, req: ApprovalRequest) -> ApprovalDecision;
}

#[derive(Debug, Clone)]
pub struct DefaultApprovalQueue {
    pub mode: HitlMode,
    pub risk_threshold: RiskLevel,
    queue: Arc<Mutex<VecDeque<ApprovalRequest>>>,
    notify: Arc<Notify>,
}

impl DefaultApprovalQueue {
    pub fn new(mode: HitlMode, risk_threshold: RiskLevel) -> Self {
        Self {
            mode,
            risk_threshold,
            queue: Arc::new(Mutex::new(VecDeque::new())),
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn enqueue(&self, req: ApprovalRequest) {
        let mut queue = self.queue.lock().await;
        queue.push_back(req);
        self.notify.notify_one();
    }

    pub async fn run_approver<H: ApprovalHandler + 'static>(self: Arc<Self>, handler: Arc<H>) {
        loop {
            let req_opt = {
                let mut queue = self.queue.lock().await;
                queue.pop_front()
            };
            match req_opt {
                Some(mut req) => {
                    let decision = handler.request_approval(req.clone()).await;
                    req.decision = Some(decision);
                    info!(
                        "Approval decision ({}): {:?}",
                        req.request_id, req.decision
                    );
                }
                None => self.notify.notified().await,
            }
        }
    }
}

pub struct ConsoleApprovalHandler;

#[async_trait]
impl ApprovalHandler for ConsoleApprovalHandler {
    async fn request_approval(&self, req: ApprovalRequest) -> ApprovalDecision {
        use ApprovalDecision::*;
        println!("\nHITL Approval Required: {:?}", req.assessment);
        println!("Enter 'a' to approve, 'r' to reject, 'm' for more info:");
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf).unwrap();
        match buf.trim() {
            "a" => Approved,
            "r" => Rejected,
            "m" => NeedsMoreInfo,
            _ => NeedsMoreInfo,
        }
    }
}
