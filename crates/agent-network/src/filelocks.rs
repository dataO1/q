//! File lock manager for concurrent access control
//!
//! Implements read-write locking semantics for files with:
//! - Multiple concurrent readers
//! - Exclusive write access
//! - Timeout handling
//! - Deadlock prevention via timeouts

use crate::error::{AgentNetworkError, AgentNetworkResult};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn, instrument};

/// Lock types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockType {
    /// Shared read lock - multiple can hold simultaneously
    Read,
    /// Exclusive write lock - only one can hold at a time
    Write,
}

impl std::fmt::Display for LockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "Read"),
            Self::Write => write!(f, "Write"),
        }
    }
}

/// Represents a lock on a file
#[derive(Debug, Clone)]
struct FileLock {
    /// ID of agent holding the lock
    agent_id: String,

    /// Type of lock held
    lock_type: LockType,

    /// List of read lock holders (for shared locks)
    readers: Vec<String>,

    /// When the lock was acquired
    acquired_at: Instant,
}

/// File lock manager with RwLock semantics
pub struct FileLockManager {
    /// Map of path -> active lock
    locks: Arc<RwLock<HashMap<PathBuf, FileLock>>>,

    /// Timeout for lock acquisition
    timeout_duration: Duration,

    /// Timeout for lock held check
    max_lock_hold_time: Duration,
}

impl FileLockManager {
    /// Create a new file lock manager
    pub fn new(timeout_seconds: u64) -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            timeout_duration: Duration::from_secs(timeout_seconds),
            max_lock_hold_time: Duration::from_secs(timeout_seconds * 2),
        }
    }

    /// Acquire a read lock on a file
    #[instrument(name = "file_read_lock_acquire", skip(self))]
    pub async fn acquire_read_lock(&self, path: PathBuf, agent_id: String) -> AgentNetworkResult<FileLockGuard> {
        let start = Instant::now();

        loop {
            if start.elapsed() > self.timeout_duration {
                return Err(AgentNetworkError::FileLockTimeout {
                    path: path.display().to_string(),
                });
            }

            let mut locks = self.locks.write().await;

            // Check if we can acquire read lock
            if let Some(lock) = locks.get(&path) {
                match lock.lock_type {
                    LockType::Read => {
                        // Add to existing read lock
                        let mut updated_lock = lock.clone();
                        updated_lock.readers.push(agent_id.clone());
                        locks.insert(path.clone(), updated_lock);

                        debug!("Read lock acquired for {} by {}", path.display(), agent_id);

                        return Ok(FileLockGuard {
                            manager: Arc::new(self.clone_for_guard()),
                            path,
                            agent_id,
                            lock_type: LockType::Read,
                        });
                    }
                    LockType::Write => {
                        // Write lock exists, must wait
                        drop(locks);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            } else {
                // Create new read lock
                locks.insert(
                    path.clone(),
                    FileLock {
                        agent_id: agent_id.clone(),
                        lock_type: LockType::Read,
                        readers: vec![agent_id.clone()],
                        acquired_at: Instant::now(),
                    },
                );

                debug!("Read lock acquired for {} by {}", path.display(), agent_id);

                return Ok(FileLockGuard {
                    manager: Arc::new(self.clone_for_guard()),
                    path,
                    agent_id,
                    lock_type: LockType::Read,
                });
            }
        }
    }

    /// Acquire a write lock on a file
    #[instrument(name = "file_write_lock_acquire", skip(self))]
    pub async fn acquire_write_lock(&self, path: PathBuf, agent_id: String) -> AgentNetworkResult<FileLockGuard> {
        let start = Instant::now();

        loop {
            if start.elapsed() > self.timeout_duration {
                warn!("Write lock timeout on: {}", path.display());
                return Err(AgentNetworkError::FileLockTimeout {
                    path: path.display().to_string(),
                });
            }

            let mut locks = self.locks.write().await;

            // Check if path is free
            if !locks.contains_key(&path) {
                locks.insert(
                    path.clone(),
                    FileLock {
                        agent_id: agent_id.clone(),
                        lock_type: LockType::Write,
                        readers: vec![],
                        acquired_at: Instant::now(),
                    },
                );

                debug!("Write lock acquired for {} by {}", path.display(), agent_id);

                return Ok(FileLockGuard {
                    manager: Arc::new(self.clone_for_guard()),
                    path,
                    agent_id,
                    lock_type: LockType::Write,
                });
            } else {
                // Path is locked, wait
                drop(locks);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }

    /// Release a lock
    pub async fn release_lock(&self, path: &PathBuf, agent_id: &str, lock_type: LockType) -> AgentNetworkResult<()> {
        let mut locks = self.locks.write().await;

        if let Some(mut lock) = locks.get_mut(path) {
            match lock_type {
                LockType::Read => {
                    // Remove from read lock holders
                    lock.readers.retain(|id| id != agent_id);

                    // If no more readers, remove the lock
                    if lock.readers.is_empty() {
                        locks.remove(path);
                    }
                }
                LockType::Write => {
                    // Remove write lock if held by this agent
                    if lock.agent_id == agent_id {
                        locks.remove(path);
                    }
                }
            }

            debug!("Lock released for {} by {}", path.display(), agent_id);
        }

        Ok(())
    }

    /// Check if a file is locked
    pub async fn is_locked(&self, path: &PathBuf) -> bool {
        let locks = self.locks.read().await;
        locks.contains_key(path)
    }

    /// Get lock count for a file
    pub async fn lock_count(&self, path: &PathBuf) -> usize {
        let locks = self.locks.read().await;
        locks
            .get(path)
            .map(|lock| {
                match lock.lock_type {
                    LockType::Read => lock.readers.len(),
                    LockType::Write => 1,
                }
            })
            .unwrap_or(0)
    }

    /// Clone for guard (internal use)
    fn clone_for_guard(&self) -> Self {
        Self {
            locks: Arc::clone(&self.locks),
            timeout_duration: self.timeout_duration,
            max_lock_hold_time: self.max_lock_hold_time,
        }
    }
}

impl Clone for FileLockManager {
    fn clone(&self) -> Self {
        Self {
            locks: Arc::clone(&self.locks),
            timeout_duration: self.timeout_duration,
            max_lock_hold_time: self.max_lock_hold_time,
        }
    }
}

/// RAII guard for file locks
pub struct FileLockGuard {
    manager: Arc<FileLockManager>,
    path: PathBuf,
    agent_id: String,
    lock_type: LockType,
}

impl Drop for FileLockGuard {
    fn drop(&mut self) {
        // Note: Cannot use async in Drop, so we use tokio::spawn to release
        let manager = Arc::clone(&self.manager);
        let path = self.path.clone();
        let agent_id = self.agent_id.clone();
        let lock_type = self.lock_type;

        tokio::spawn(async move {
            let _ = manager.release_lock(&path, &agent_id, lock_type).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_lock_creation() {
        let manager = FileLockManager::new(10);
        let path = PathBuf::from("/tmp/test.txt");

        let result = manager.acquire_read_lock(path.clone(), "agent1".to_string()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_write_lock_exclusivity() {
        let manager = FileLockManager::new(1); // 1 second timeout
        let path = PathBuf::from("/tmp/test2.txt");

        let write_lock = manager
            .acquire_write_lock(path.clone(), "agent1".to_string())
            .await;
        assert!(write_lock.is_ok());

        // Try to acquire another write lock (should timeout)
        let result = manager
            .acquire_write_lock(path.clone(), "agent2".to_string())
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_lock_type_display() {
        assert_eq!(LockType::Read.to_string(), "Read");
        assert_eq!(LockType::Write.to_string(), "Write");
    }
}
