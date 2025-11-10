//! Shared context registry used by multiple agents in the orchestration

use std::collections::HashMap;

pub struct SharedContext {
    interfaces: HashMap<String, String>,  // Interface name → definition
    type_registry: HashMap<String, String>, // Type name → definition
    constraints: Vec<String>,
}

impl SharedContext {
    pub fn new() -> Self {
        Self {
            interfaces: HashMap::new(),
            type_registry: HashMap::new(),
            constraints: Vec::new(),
        }
    }

    pub fn add_interface(&mut self, name: String, definition: String) {
        self.interfaces.insert(name, definition);
    }

    pub fn add_type(&mut self, name: String, definition: String) {
        self.type_registry.insert(name, definition);
    }

    pub fn add_constraint(&mut self, constraint: String) {
        self.constraints.push(constraint);
    }
}
