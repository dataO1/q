use ai_agent_common::*;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher, Config};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tokio::sync::mpsc;
use anyhow::{Context, Result};

/// File system watcher with .gitignore and configurable filtering support
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
    watched_paths: Vec<PathBuf>,
    filters: IndexingFilters,
    gitignore_matchers: HashMap<PathBuf, Gitignore>,
}

impl FileWatcher {
    /// Create a new file watcher with configuration
    pub fn new(paths: Vec<PathBuf>, filters: IndexingFilters) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();

        let watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Err(e) = tx.send(res) {
                    tracing::error!("Failed to send file event: {}", e);
                }
            },
            Config::default()
                .with_poll_interval(std::time::Duration::from_secs(2)),
        )
        .context("Failed to create file watcher")?;

        let mut file_watcher = Self {
            watcher,
            rx,
            watched_paths: paths.clone(),
            filters,
            gitignore_matchers: HashMap::new(),
        };

        // Load .gitignore files if enabled
        if file_watcher.filters.respect_gitignore {
            file_watcher.load_gitignore_files(&paths)?;
        }

        Ok(file_watcher)
    }

    /// Load .gitignore files from watched directories
    fn load_gitignore_files(&mut self, paths: &[PathBuf]) -> Result<()> {
        for path in paths {
            if let Ok(gitignore) = self.build_gitignore(path) {
                self.gitignore_matchers.insert(path.clone(), gitignore);
                tracing::info!("Loaded .gitignore for: {}", path.display());
            }
        }
        Ok(())
    }

    /// Build gitignore matcher for a directory
    fn build_gitignore(&self, root: &Path) -> Result<Gitignore> {
        let mut builder = GitignoreBuilder::new(root);

        // Add .gitignore file if it exists
        let gitignore_path = root.join(".gitignore");
        if gitignore_path.exists() {
            builder.add(gitignore_path);
        }

        // Add custom ignore patterns from config
        for pattern in &self.filters.custom_ignores {
            builder.add_line(None, pattern)?;
        }

        Ok(builder.build()?)
    }

    /// Start watching all configured paths
    pub fn start_watching(&mut self) -> Result<()> {
        for path in &self.watched_paths {
            if path.exists() {
                self.watcher
                    .watch(path, RecursiveMode::Recursive)
                    .context(format!("Failed to watch path: {}", path.display()))?;
                tracing::info!("Watching path: {}", path.display());
            } else {
                tracing::warn!("Path does not exist, skipping: {}", path.display());
            }
        }
        Ok(())
    }

    /// Add a new path to watch
    pub fn add_path(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        self.watcher
            .watch(path, RecursiveMode::Recursive)
            .context(format!("Failed to add watch for: {}", path.display()))?;

        // Load .gitignore for new path
        if self.filters.respect_gitignore {
            if let Ok(gitignore) = self.build_gitignore(path) {
                self.gitignore_matchers.insert(path.to_path_buf(), gitignore);
            }
        }

        self.watched_paths.push(path.to_path_buf());
        tracing::info!("Added watch for: {}", path.display());
        Ok(())
    }

    /// Remove a path from watching
    pub fn remove_path(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        self.watcher
            .unwatch(path)
            .context(format!("Failed to remove watch for: {}", path.display()))?;

        self.watched_paths.retain(|p| p != path);
        self.gitignore_matchers.remove(path);
        tracing::info!("Removed watch for: {}", path.display());
        Ok(())
    }

    /// Watch for file changes (async event loop)
    pub async fn watch(&mut self) -> Result<FileEvent> {
        loop {
            match self.rx.recv().await {
                Some(Ok(event)) => {
                    if let Some(file_event) = self.process_event(event) {
                        return Ok(file_event);
                    }
                }
                Some(Err(e)) => {
                    tracing::error!("File watcher error: {}", e);
                }
                None => {
                    anyhow::bail!("File watcher channel closed unexpectedly");
                }
            }
        }
    }

    /// Process raw notify event into FileEvent
    fn process_event(&self, event: Event) -> Option<FileEvent> {
        use notify::EventKind;
        tracing::error!("File watcher got event: {:?}", event);

        match event.kind {
            EventKind::Create(_) => {
                for path in event.paths {
                    if self.should_process_path(&path) {
                        return Some(FileEvent::Created(path));
                    }
                }
            }
            EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                for path in event.paths {
                    if self.should_process_path(&path) {
                        return Some(FileEvent::Modified(path));
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    if self.should_process_path(&path) {
                        return Some(FileEvent::Deleted(path));
                    }
                }
            }
            _ => {}
        }

        None
    }

    /// Check if a path should be processed based on all filters
    fn should_process_path(&self, path: &Path) -> bool {
        // Only process files, not directories
        if !path.is_file() {
            return false;
        }

        // Check file size limit
        if let Some(max_size) = self.filters.max_file_size {
            if let Ok(metadata) = std::fs::metadata(path) {
                if metadata.len() > max_size {
                    tracing::debug!("Skipping file (too large): {}", path.display());
                    return false;
                }
            }
        }

        // Check if hidden file (unless include_hidden is true)
        if !self.filters.include_hidden {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') && name != ".config" {
                    return false;
                }
            }
        }

        // Check ignored directories
        for component in path.components() {
            if let Some(name) = component.as_os_str().to_str() {
                if self.filters.ignore_dirs.contains(&name.to_string()) {
                    return false;
                }
            }
        }

        // Check ignored file extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if self.filters.ignore_extensions.contains(&ext.to_string()) {
                return false;
            }
        }

        // Check .gitignore rules
        if self.filters.respect_gitignore {
            if self.is_ignored_by_gitignore_internal(path) {
                return false;
            }
        }

        true
    }

    /// Check if path is ignored by .gitignore
    fn is_ignored_by_gitignore_internal(&self, path: &Path) -> bool {
        // Find the appropriate gitignore matcher for this path
        for (root, gitignore) in &self.gitignore_matchers {
            if path.starts_with(root) {
                // Get relative path from root
                if let Ok(relative) = path.strip_prefix(root) {
                    let matched = gitignore.matched(relative, path.is_dir());
                    if matched.is_ignore() {
                        tracing::debug!("Ignored by .gitignore: {}", path.display());
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// File system event types
#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}



impl FileEvent {
    pub fn path(&self) -> &Path {
        match self {
            FileEvent::Created(p) => p,
            FileEvent::Modified(p) => p,
            FileEvent::Deleted(p) => p,
        }
    }

    pub fn event_type(&self) -> &str {
        match self {
            FileEvent::Created(_) => "created",
            FileEvent::Modified(_) => "modified",
            FileEvent::Deleted(_) => "deleted",
        }
    }
}

// Test-only access via cfg(test)
impl FileWatcher{
    // Test-only methods (only in test builds)
    pub fn watched_path_count(&self) -> usize {
        self.watched_paths.len()
    }

    pub fn is_watching(&self, path: &Path) -> bool {
        self.watched_paths.iter().any(|p| p == path)
    }

    pub fn is_ignored_by_gitignore(&self, path: &Path) -> bool {
        self.is_ignored_by_gitignore_internal(path)
    }

    pub fn get_watched_paths(&self) -> &Vec<PathBuf> {
        &self.watched_paths
    }
}
// Test-only access via cfg(test)
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create test filters
    fn test_filters() -> IndexingFilters {
        IndexingFilters {
            respect_gitignore: true,
            include_hidden: false,
            ignore_dirs: vec!["node_modules".to_string(), "target".to_string()],
            ignore_extensions: vec!["lock".to_string(), "log".to_string()],
            custom_ignores: vec!["*.tmp".to_string()],
            max_file_size: Some(1024 * 1024), // 1MB
        }
    }

    #[test]
    fn test_file_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let watcher = FileWatcher::new(vec![path], test_filters());
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_should_process_regular_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let watcher = FileWatcher::new(vec![temp_dir.path().to_path_buf()], test_filters()).unwrap();

        assert!(watcher.should_process_path(&test_file));
    }

    #[test]
    fn test_filter_hidden_files() {
        let temp_dir = TempDir::new().unwrap();
        let hidden_file = temp_dir.path().join(".hidden");
        fs::write(&hidden_file, "content").unwrap();

        let watcher = FileWatcher::new(vec![temp_dir.path().to_path_buf()], test_filters()).unwrap();

        assert!(!watcher.should_process_path(&hidden_file));
    }

    #[test]
    fn test_filter_ignored_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let lock_file = temp_dir.path().join("Cargo.lock");
        fs::write(&lock_file, "content").unwrap();

        let watcher = FileWatcher::new(vec![temp_dir.path().to_path_buf()], test_filters()).unwrap();

        assert!(!watcher.should_process_path(&lock_file));
    }

    #[test]
    fn test_filter_ignored_directories() {
        let temp_dir = TempDir::new().unwrap();
        let node_modules = temp_dir.path().join("node_modules");
        fs::create_dir(&node_modules).unwrap();
        let file_in_nm = node_modules.join("package.json");
        fs::write(&file_in_nm, "{}").unwrap();

        let watcher = FileWatcher::new(vec![temp_dir.path().to_path_buf()], test_filters()).unwrap();

        assert!(!watcher.should_process_path(&file_in_nm));
    }

    #[test]
    fn test_include_hidden_when_configured() {
        let temp_dir = TempDir::new().unwrap();
        let hidden_file = temp_dir.path().join(".hidden");
        fs::write(&hidden_file, "content").unwrap();

        let mut filters = test_filters();
        filters.include_hidden = true;

        let watcher = FileWatcher::new(vec![temp_dir.path().to_path_buf()], filters).unwrap();

        assert!(watcher.should_process_path(&hidden_file));
    }

    #[test]
    fn test_file_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let large_file = temp_dir.path().join("large.txt");

        // Create file larger than 1MB limit
        let large_content = vec![b'x'; 2 * 1024 * 1024];
        fs::write(&large_file, large_content).unwrap();

        let watcher = FileWatcher::new(vec![temp_dir.path().to_path_buf()], test_filters()).unwrap();

        assert!(!watcher.should_process_path(&large_file));
    }

    #[test]
    fn test_file_event_types() {
        let path = PathBuf::from("/test/file.txt");

        let created = FileEvent::Created(path.clone());
        assert_eq!(created.event_type(), "created");
        assert_eq!(created.path(), path.as_path());

        let modified = FileEvent::Modified(path.clone());
        assert_eq!(modified.event_type(), "modified");

        let deleted = FileEvent::Deleted(path.clone());
        assert_eq!(deleted.event_type(), "deleted");
    }

}
