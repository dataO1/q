use ai_agent_common::*;
use std::path::PathBuf;

pub struct MetadataTracker;

impl MetadataTracker {
    pub fn extract_file_references(&self, message: &str) -> Vec<PathBuf> {
        todo!("Extract file paths from text")
    }

    pub fn extract_code_snippets(&self, message: &str) -> Vec<String> {
        todo!("Extract code blocks")
    }

    pub fn detect_topics(&self, messages: &[Message]) -> Vec<String> {
        todo!("Cluster topics")
    }
}
