pub mod file_locks;
pub mod shared_context;
pub mod conflict;
use anyhow::Result;

use ai_agent_common::*;

/// Coordination layer managing shared resources between agents
pub struct CoordinationLayer {
    file_locks: file_locks::FileLockManager,
    shared_context: shared_context::SharedContext,
    conflict_detector: conflict::ConflictDetector,
}

impl CoordinationLayer {
    pub fn new() -> Self {
        Self {
            file_locks: file_locks::FileLockManager::new(),
            shared_context: shared_context::SharedContext::new(),
            conflict_detector: conflict::ConflictDetector::new(),
        }
    }

    pub async fn acquire_file_lock(
        &self,
        path: std::path::PathBuf,
        task_id: TaskId,
    ) -> Result<()> {
        self.file_locks.acquire_lock(path, task_id).await
    }

    pub async fn release_file_lock(&self, path: &std::path::PathBuf) -> Result<()> {
        self.file_locks.release_lock(path).await
    }

    pub fn check_conflicts(&self, files: &[std::path::PathBuf]) -> bool {
        self.conflict_detector.check_conflict(files)
    }
}
