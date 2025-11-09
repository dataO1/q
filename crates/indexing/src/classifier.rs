use ai_agent_common::*;
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait PathClassifierTrait: Send + Sync {
    fn priority(&self) -> u8;
    async fn classify(&self, path: &Path) -> Option<CollectionTier>;
}

pub struct PathClassifier {
    classifiers: Vec<Box<dyn PathClassifierTrait>>,
}

impl PathClassifier {
    pub fn new(config: &IndexingConfig) -> Self {
        todo!("Build classifier chain with priority ordering")
    }

    pub async fn classify(&self, path: &Path) -> Result<CollectionTier> {
        todo!("Run through classifier chain")
    }
}

// Individual classifiers
pub struct SystemPathClassifier;
pub struct PersonalPathClassifier {
    personal_dirs: Vec<String>,
}
pub struct WorkspacePathClassifier {
    workspace_dirs: Vec<String>,
}
pub struct DependenciesClassifier;

#[async_trait]
impl PathClassifierTrait for SystemPathClassifier {
    fn priority(&self) -> u8 { 10 }
    async fn classify(&self, path: &Path) -> Option<CollectionTier> {
        todo!("Classify system paths")
    }
}

// Implement other classifiers...
