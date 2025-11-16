//! Shared context management for agents

use std::collections::HashMap;

pub struct SharedContext {
    context_data: HashMap<String, String>,
}

impl SharedContext {
    pub fn new() -> Self {
        Self {
            context_data: HashMap::new(),
        }
    }

    /// Set context value
    pub fn set(&mut self, key: String, value: String) {
        self.context_data.insert(key, value);
    }

    /// Get context value
    pub fn get(&self, key: &str) -> Option<&String> {
        self.context_data.get(key)
    }

    /// Remove context value
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.context_data.remove(key)
    }

    /// Clear all context
    pub fn clear(&mut self) {
        self.context_data.clear();
    }

    /// Get all context data
    pub fn get_all(&self) -> &HashMap<String, String> {
        &self.context_data
    }
}

impl Default for SharedContext {
    fn default() -> Self {
        Self::new()
    }
}
