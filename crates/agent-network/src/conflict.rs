//! Conflict detection and resolution for concurrent operations
//!
//! Detects and resolves conflicts when multiple agents try to access
//! or modify the same resources simultaneously.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::error::AgentNetworkResult;

/// Resource access type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessType {
    Read,
    Write,
    Execute,
}

impl std::fmt::Display for AccessType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "Read"),
            Self::Write => write!(f, "Write"),
            Self::Execute => write!(f, "Execute"),
        }
    }
}

/// Represents an active resource access
#[derive(Debug, Clone)]
pub struct ResourceAccess {
    pub resource_id: String,
    pub agent_id: String,
    pub access_type: AccessType,
}

/// Conflict detection and resolution
pub struct ConflictResolver {
    /// Map of resource -> active accesses
    active_accesses: Arc<RwLock<HashMap<String, Vec<ResourceAccess>>>>,
}

impl ConflictResolver {
    /// Create a new conflict resolver
    pub fn new() -> Self {
        Self {
            active_accesses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if an access would conflict with existing accesses
    pub async fn check_conflict(
        &self,
        resource_id: &str,
        agent_id: &str,
        access_type: AccessType,
    ) -> AgentNetworkResult<bool> {
        let accesses = self.active_accesses.read().await;

        if let Some(resource_accesses) = accesses.get(resource_id) {
            // Check for conflicts
            for existing in resource_accesses {
                if existing.agent_id == agent_id {
                    // Same agent, no conflict
                    continue;
                }

                match (existing.access_type, access_type) {
                    // Write always conflicts with anything
                    (AccessType::Write, _) | (_, AccessType::Write) => {
                        debug!(
                            "Conflict detected: {} wants {} access to {}, but {} has {} access",
                            agent_id,
                            access_type,
                            resource_id,
                            existing.agent_id,
                            existing.access_type
                        );
                        return Ok(true);
                    }
                    // Multiple reads are OK
                    (AccessType::Read, AccessType::Read) => {}
                    _ => {}
                }
            }
        }

        Ok(false)
    }

    /// Register a resource access
    pub async fn register_access(
        &self,
        resource_id: String,
        agent_id: String,
        access_type: AccessType,
    ) -> AgentNetworkResult<()> {
        let mut accesses = self.active_accesses.write().await;

        accesses
            .entry(resource_id.clone())
            .or_insert_with(Vec::new)
            .push(ResourceAccess {
                resource_id,
                agent_id,
                access_type,
            });

        Ok(())
    }

    /// Unregister a resource access
    pub async fn unregister_access(
        &self,
        resource_id: &str,
        agent_id: &str,
    ) -> AgentNetworkResult<()> {
        let mut accesses = self.active_accesses.write().await;

        if let Some(resource_accesses) = accesses.get_mut(resource_id) {
            resource_accesses.retain(|access| access.agent_id != agent_id);

            if resource_accesses.is_empty() {
                accesses.remove(resource_id);
            }
        }

        Ok(())
    }

    /// Get all active accesses for a resource
    pub async fn get_active_accesses(&self, resource_id: &str) -> Vec<ResourceAccess> {
        let accesses = self.active_accesses.read().await;
        accesses
            .get(resource_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get total active accesses across all resources
    pub async fn get_total_accesses(&self) -> usize {
        let accesses = self.active_accesses.read().await;
        accesses.values().map(|v| v.len()).sum()
    }

    /// Detect deadlock potential
    pub async fn detect_potential_deadlock(&self) -> Vec<(String, String)> {
        let accesses = self.active_accesses.read().await;
        let mut conflicts = vec![];

        for (resource_id, resource_accesses) in accesses.iter() {
            let mut write_holder = None;
            let mut read_holders = vec![];

            for access in resource_accesses {
                match access.access_type {
                    AccessType::Write => write_holder = Some(access.agent_id.clone()),
                    AccessType::Read => read_holders.push(access.agent_id.clone()),
                    _ => {}
                }
            }

            // Write holder blocking readers is a potential issue
            if let Some(writer) = write_holder {
                if !read_holders.is_empty() {
                    conflicts.push((writer, read_holders.join(", ")));
                }
            }
        }

        conflicts
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ConflictResolver {
    fn clone(&self) -> Self {
        Self {
            active_accesses: Arc::clone(&self.active_accesses),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conflict_detection() {
        let resolver = ConflictResolver::new();

        // Agent 1 writes
        resolver
            .register_access("file1".to_string(), "agent1".to_string(), AccessType::Write)
            .await
            .ok();

        // Agent 2 tries to write (should conflict)
        let conflict = resolver
            .check_conflict("file1", "agent2", AccessType::Write)
            .await
            .unwrap();
        assert!(conflict);
    }

    #[tokio::test]
    async fn test_read_read_no_conflict() {
        let resolver = ConflictResolver::new();

        // Agent 1 reads
        resolver
            .register_access("file1".to_string(), "agent1".to_string(), AccessType::Read)
            .await
            .ok();

        // Agent 2 reads (should NOT conflict)
        let conflict = resolver
            .check_conflict("file1", "agent2", AccessType::Read)
            .await
            .unwrap();
        assert!(!conflict);
    }
}
