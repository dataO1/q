
//! Conflict detection between agents acting on files or tasks

use std::collections::HashSet;
use std::path::PathBuf;
use ai_agent_common::*;

pub struct ConflictDetector {
    locked_files: HashSet<PathBuf>,
}

impl ConflictDetector {
    pub fn new() -> Self {
        Self {
            locked_files: HashSet::new(),
        }
    }

    pub fn check_conflict(&self, files: &[PathBuf]) -> bool {
        files.iter().any(|f| self.locked_files.contains(f))
    }

    pub fn add_lock(&mut self, files: &[PathBuf]) {
        for f in files {
            self.locked_files.insert(f.clone());
        }
    }

    pub fn remove_lock(&mut self, files: &[PathBuf]) {
        for f in files {
            self.locked_files.remove(f);
        }
    }
}
