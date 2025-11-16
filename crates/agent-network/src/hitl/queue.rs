//! HITL task queue

use crate::{
    hitl::{HitlRequest, HitlResponse},
};
use std::collections::VecDeque;
use ai_agent_common::AgentResult;
use tokio::sync::RwLock;

pub struct HitlQueue {
    queue: RwLock<VecDeque<HitlRequest>>,
}

impl HitlQueue {
    pub fn new() -> Self {
        Self {
            queue: RwLock::new(VecDeque::new()),
        }
    }

    /// Add request to queue
    pub async fn enqueue(&self, request: HitlRequest) -> AgentResult<()> {
        let mut queue = self.queue.write().await;
        queue.push_back(request);
        tracing::info!("HITL request enqueued: queue length {}", queue.len());
        Ok(())
    }

    /// Get next request from queue
    pub async fn dequeue(&self) -> Option<HitlRequest> {
        let mut queue = self.queue.write().await;
        queue.pop_front()
    }

    /// Get queue length
    pub async fn len(&self) -> usize {
        let queue = self.queue.read().await;
        queue.len()
    }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        let queue = self.queue.read().await;
        queue.is_empty()
    }
}

impl Default for HitlQueue {
    fn default() -> Self {
        Self::new()
    }
}
