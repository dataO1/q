//! Shared context management for multi-agent coordination
//!
//! Provides a thread-safe, shared context that agents can use to
//! read and write shared state during workflow execution.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Shared execution context for agents
pub struct SharedContext {
    /// Key-value store for shared state
    context_data: Arc<RwLock<HashMap<String, ContextValue>>>,

    /// Access control for context entries
    access_control: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

/// A value that can be stored in shared context
#[derive(Debug, Clone)]
pub enum ContextValue {
    String(String),
    Number(f64),
    Boolean(bool),
    List(Vec<ContextValue>),
    Map(HashMap<String, ContextValue>),
}

impl std::fmt::Display for ContextValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(s) => write!(f, "{}", s),
            Self::Number(n) => write!(f, "{}", n),
            Self::Boolean(b) => write!(f, "{}", b),
            Self::List(l) => write!(f, "[{} items]", l.len()),
            Self::Map(m) => write!(f, "{{map with {} keys}}", m.len()),
        }
    }
}

impl ContextValue {
    /// Convert to string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to number
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Convert to boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

impl SharedContext {
    /// Create new shared context
    pub fn new() -> Self {
        Self {
            context_data: Arc::new(RwLock::new(HashMap::new())),
            access_control: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set a context value
    pub async fn set(&self, key: String, value: ContextValue) {
        let mut data = self.context_data.write().await;
        data.insert(key, value);
    }

    /// Get a context value
    pub async fn get(&self, key: &str) -> Option<ContextValue> {
        let data = self.context_data.read().await;
        data.get(key).cloned()
    }

    /// Remove a context value
    pub async fn remove(&self, key: &str) -> Option<ContextValue> {
        let mut data = self.context_data.write().await;
        data.remove(key)
    }

    /// Check if key exists
    pub async fn contains_key(&self, key: &str) -> bool {
        let data = self.context_data.read().await;
        data.contains_key(key)
    }

    /// Clear all context
    pub async fn clear(&self) {
        let mut data = self.context_data.write().await;
        data.clear();
    }

    /// Get all keys
    pub async fn keys(&self) -> Vec<String> {
        let data = self.context_data.read().await;
        data.keys().cloned().collect()
    }

    /// Get all context data as a clone
    pub async fn get_all(&self) -> HashMap<String, ContextValue> {
        let data = self.context_data.read().await;
        data.clone()
    }

    /// Set access control for a key (which agents can read)
    pub async fn set_read_access(&self, key: String, agent_ids: Vec<String>) {
        let mut ac = self.access_control.write().await;
        ac.insert(key, agent_ids);
    }

    /// Check read access for an agent
    pub async fn can_read(&self, key: &str, agent_id: &str) -> bool {
        let ac = self.access_control.read().await;

        match ac.get(key) {
            Some(allowed_agents) => allowed_agents.contains(&agent_id.to_string()),
            None => true, // No restriction = all can read
        }
    }

    /// Merge context from another source
    pub async fn merge(&self, other: &HashMap<String, ContextValue>) {
        let mut data = self.context_data.write().await;
        for (key, value) in other {
            data.insert(key.clone(), value.clone());
        }
    }

    /// Get context size in terms of number of entries
    pub async fn size(&self) -> usize {
        let data = self.context_data.read().await;
        data.len()
    }

    /// Export as string for logging
    pub async fn to_debug_string(&self) -> String {
        let data = self.context_data.read().await;

        let entries: Vec<String> = data
            .iter()
            .take(10)
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect();

        if data.len() > 10 {
            format!("{{{}\n... and {} more}}", entries.join(",\n"), data.len() - 10)
        } else {
            format!("{{{}}}", entries.join(", "))
        }
    }
}

impl Default for SharedContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SharedContext {
    fn clone(&self) -> Self {
        Self {
            context_data: Arc::clone(&self.context_data),
            access_control: Arc::clone(&self.access_control),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get() {
        let ctx = SharedContext::new();

        ctx.set("key1".to_string(), ContextValue::String("value1".to_string())).await;

        let val = ctx.get("key1").await;
        assert!(val.is_some());
        assert_eq!(val.unwrap().as_string(), Some("value1"));
    }

    #[tokio::test]
    async fn test_remove() {
        let ctx = SharedContext::new();

        ctx.set("key1".to_string(), ContextValue::Number(42.0)).await;
        assert!(ctx.contains_key("key1").await);

        let removed = ctx.remove("key1").await;
        assert!(removed.is_some());
        assert!(!ctx.contains_key("key1").await);
    }

    #[tokio::test]
    async fn test_access_control() {
        let ctx = SharedContext::new();

        ctx.set_read_access("secret".to_string(), vec!["agent1".to_string()]).await;

        assert!(ctx.can_read("secret", "agent1").await);
        assert!(!ctx.can_read("secret", "agent2").await);
    }

    #[test]
    fn test_context_value_conversions() {
        let val_string = ContextValue::String("hello".to_string());
        assert_eq!(val_string.as_string(), Some("hello"));
        assert_eq!(val_string.as_number(), None);

        let val_number = ContextValue::Number(3.14);
        assert_eq!(val_number.as_number(), Some(3.14));
        assert_eq!(val_number.as_string(), None);
    }
}
