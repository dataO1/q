//! File lock manager for concurrent access control

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use ai_agent_common::AgentNetworkError;
use tokio::sync::RwLock;
use tokio::time::timeout;
use ai_agent_common::types::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum LockType {
    Read,
    Write,
}

pub struct FileLockManager {
    locks: Arc<RwLock<HashMap<PathBuf, FileLock>>>,
    timeout_duration: Duration,
}

#[derive(Debug, Clone)]
struct FileLock {
    agent_id: String,
    lock_type: LockType,
    readers: Vec<String>,
}

impl FileLockManager {
    pub fn new(timeout_seconds: u64) -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            timeout_duration: Duration::from_secs(timeout_seconds),
        }
    }

    /// Acquire read lock on file
    pub async fn acquire_read_lock(&self, path: PathBuf, agent_id: String) -> Result<()> {
        let result = timeout(
            self.timeout_duration,
            self.try_acquire_read_lock(path.clone(), agent_id.clone())
        ).await;

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(AgentNetworkError::FileLock(
                format!("Timeout acquiring read lock on {:?}", path)
            )),
        }
    }

    /// Acquire write lock on file
    pub async fn acquire_write_lock(&self, path: PathBuf, agent_id: String) -> Result<()> {
        let result = timeout(
            self.timeout_duration,
            self.try_acquire_write_lock(path.clone(), agent_id.clone())
        ).await;

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(AgentNetworkError::FileLock(
                format!("Timeout acquiring write lock on {:?}", path)
            )),
        }
    }

    /// Release lock on file
    pub async fn release_lock(&self, path: &PathBuf, agent_id: &str) -> Result<()> {
        let mut locks = self.locks.write().await;

        if let Some(lock) = locks.get_mut(path) {
            if lock.agent_id == agent_id {
                locks.remove(path);
            } else {
                lock.readers.retain(|id| id != agent_id);
                if lock.readers.is_empty() && lock.lock_type == LockType::Read {
                    locks.remove(path);
                }
            }
        }

        Ok(())
    }

    async fn try_acquire_read_lock(&self, path: PathBuf, agent_id: String) -> Result<()> {
        loop {
            let mut locks = self.locks.write().await;

            if let Some(lock) = locks.get_mut(&path) {
                if lock.lock_type == LockType::Read {
                    lock.readers.push(agent_id.clone());
                    return Ok(());
                }
                // Write lock exists, wait
                drop(locks);
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else {
                locks.insert(path.clone(), FileLock {
                    agent_id: agent_id.clone(),
                    lock_type: LockType::Read,
                    readers: vec![agent_id],
                });
                return Ok(());
            }
        }
    }

    async fn try_acquire_write_lock(&self, path: PathBuf, agent_id: String) -> Result<()> {
        loop {
            let mut locks = self.locks.write().await;

            if locks.contains_key(&path) {
                // Any lock exists, wait
                drop(locks);
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else {
                locks.insert(path.clone(), FileLock {
                    agent_id: agent_id.clone(),
                    lock_type: LockType::Write,
                    readers: vec![],
                });
                return Ok(());
            }
        }
    }
}
