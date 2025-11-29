use ai_agent_common::*;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use anyhow::Result;
use mime_guess;
use std::collections::HashMap;
use tokio::fs;
use tokio_stream::wrappers::ReadDirStream;
use tokio_stream::StreamExt;

/// Mapping file extensions to language names (no longer restricted to enum)
fn extension_to_language(ext: &str) -> Option<String> {
    match ext {
        "rs" => Some("Rust".to_string()),
        "py" => Some("Python".to_string()),
        "js" => Some("JavaScript".to_string()),
        "ts" => Some("TypeScript".to_string()),
        "java" => Some("Java".to_string()),
        "c" => Some("C".to_string()),
        "cpp" | "cc" | "cxx" | "hpp" | "h" => Some("Cpp".to_string()),
        "go" => Some("Go".to_string()),
        "hs" => Some("Haskell".to_string()),
        "lua" => Some("Lua".to_string()),
        "yaml" | "yml" => Some("YAML".to_string()),
        "toml" => Some("TOML".to_string()), // Added TOML support
        "sh" | "bash" => Some("Bash".to_string()),
        "html" | "htm" => Some("HTML".to_string()),
        "json" => Some("JSON".to_string()),
        "rb" => Some("Ruby".to_string()),
        "adoc" => Some("Asciidoc".to_string()),
        "xml" => Some("XML".to_string()),
        "md" => Some("Markdown".to_string()),
        "yarn" => Some("Yarn".to_string()),
        _ => None,
    }
}

/// Detects language distribution within project_root by counting mapped file extensions
pub async fn detect_languages(project_root: &Path) -> Vec<(String, f32)> {
    let mut language_counts: HashMap<String, usize> = HashMap::new();
    let mut total_files = 0;

    let mut dirs_to_visit = vec![project_root.to_path_buf()];

    while let Some(dir_path) = dirs_to_visit.pop() {
        let read_dir = match fs::read_dir(&dir_path).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        let mut stream = ReadDirStream::new(read_dir);
        while let Some(entry_res) = stream.next().await {
            if let Ok(entry) = entry_res {
                let path = entry.path();
                if path.is_dir() {
                    dirs_to_visit.push(path);
                } else {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if let Some(lang) = extension_to_language(ext) {
                            *language_counts.entry(lang).or_insert(0) += 1;
                            total_files += 1;
                        }
                    }
                }
            }
        }
    }

    // Calculate percentage distribution
    let mut distribution = Vec::new();
    if total_files > 0 {
        for (lang, count) in language_counts {
            let percentage = (count as f32) / (total_files as f32);
            distribution.push((lang, percentage));
        }
    }

    distribution.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    distribution
}

/// Trait for path classification
#[async_trait]
pub trait PathClassifierTrait: Send + Sync {
    /// Priority of this classifier (higher = checked first)
    fn priority(&self) -> u8;

    /// Attempt to classify a path, returns Some(tier) if matched
    async fn classify(&self, path: &Path) -> Option<CollectionTier>;
}

/// Main path classifier that runs a chain of classifiers
pub struct PathClassifier {
    classifiers: Vec<Box<dyn PathClassifierTrait>>,
}

impl PathClassifier {
    /// Create a new classifier chain from configuration
    pub fn new(config: &IndexingConfig) -> Self {
        let mut classifiers: Vec<Box<dyn PathClassifierTrait>> = vec![
            Box::new(SystemPathClassifier::new(&config.system_paths)),       // 100
            Box::new(PersonalPathClassifier::new(&config.personal_paths)),   // 80
            Box::new(WorkspacePathClassifier::new(&config.workspace_paths)), // 60
        ];

        classifiers.sort_by(|a, b| b.priority().cmp(&a.priority()));
        Self { classifiers }
    }

    /// Classify a path through the classifier chain
    pub async fn classify(&self, path: &Path) -> Result<ClassificationResult> {
        // Run through classifier chain
        for classifier in &self.classifiers {
            if let Some(tier) = classifier.classify(path).await {
                return Ok(ClassificationResult {
                    tier,
                    languages: detect_languages(path).await,
                    file_type: detect_file_type(path),
                    project_root: find_project_root(path),
                    mime_type: detect_mime_type(path),
                });
            }
        }

        // Default to workspace if no match
        Ok(ClassificationResult {
            tier: CollectionTier::Workspace,
            languages: detect_languages(path).await,
            file_type: detect_file_type(path),
            project_root: find_project_root(path),
            mime_type: detect_mime_type(path),
        })
    }
}

/// Result of path classification
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub tier: CollectionTier,
    pub languages: Vec<(String, f32)>,
    pub file_type: String,
    pub project_root: Option<PathBuf>,
    pub mime_type: Option<String>,
}

// ============================================================================
// Individual Classifiers
// ============================================================================

/// Classifies system paths (/usr, /etc, /lib, etc.)
pub struct SystemPathClassifier {
    system_prefixes: Vec<PathBuf>,
}

impl SystemPathClassifier {
    pub fn new(configured_paths: &[PathBuf]) -> Self {
        let mut prefixes = configured_paths.to_vec();

        // Add common system paths
        let common_system_paths = [
            "/usr/share/man",
            "/usr/share/doc",
            "/usr/local/share/man",
            "/usr/include",
            "/etc",
            "/lib",
            "/usr/lib",
        ];

        for path_str in &common_system_paths {
            let path = PathBuf::from(path_str);
            if !prefixes.contains(&path) {
                prefixes.push(path);
            }
        }

        Self { system_prefixes: prefixes }
    }
}

#[async_trait]
impl PathClassifierTrait for SystemPathClassifier {
    fn priority(&self) -> u8 { 100 } // Highest priority

    async fn classify(&self, path: &Path) -> Option<CollectionTier> {
        for prefix in &self.system_prefixes {
            if path.starts_with(prefix) {
                return Some(CollectionTier::System);
            }
        }
        None
    }
}

/// Classifies personal/documents paths
pub struct PersonalPathClassifier {
    personal_dirs: Vec<PathBuf>,
}

impl PersonalPathClassifier {
    pub fn new(configured_paths: &[PathBuf]) -> Self {
        let mut dirs = configured_paths.to_vec();

        // Add common personal directories (expanded from ~)
        if let Some(home) = dirs::home_dir() {
            let common_personal = [
                "Documents", "notes", ".config",
                "Desktop", "Downloads", "writings",
            ];

            for dir_name in &common_personal {
                let path = home.join(dir_name);
                if !dirs.contains(&path) {
                    dirs.push(path);
                }
            }
        }

        Self { personal_dirs: dirs }
    }
}

#[async_trait]
impl PathClassifierTrait for PersonalPathClassifier {
    fn priority(&self) -> u8 { 80 }

    async fn classify(&self, path: &Path) -> Option<CollectionTier> {
        for dir in &self.personal_dirs {
            if path.starts_with(dir) {
                return Some(CollectionTier::Personal);
            }
        }
        None
    }
}

/// Classifies workspace/development paths
pub struct WorkspacePathClassifier {
    workspace_dirs: Vec<PathBuf>,
}

impl WorkspacePathClassifier {
    pub fn new(configured_paths: &[PathBuf]) -> Self {
        Self {
            workspace_dirs: configured_paths.to_vec(),
        }
    }
   /// Check if path is in a dependency directory
    fn is_in_dependency_dir(&self, path: &Path) -> bool {
        let dependency_indicators = [
            "node_modules",
            "vendor",
            ".cargo/registry",
            ".cargo/git",
            "site-packages",
            "venv",
            ".venv",
            "env",
            "target/debug/deps",
            "target/release/deps",
        ];

        for component in path.components() {
            if let Some(name) = component.as_os_str().to_str() {
                for indicator in &dependency_indicators {
                    if name == *indicator || name.contains(indicator) {
                        return true;
                    }
                }
            }
        }

        false
    }
}

#[async_trait]
impl PathClassifierTrait for WorkspacePathClassifier {
    fn priority(&self) -> u8 { 60 }

    async fn classify(&self, path: &Path) -> Option<CollectionTier> {
        for dir in &self.workspace_dirs {
            if path.starts_with(dir) {
                // Check if it's actually a dependency within workspace
                if self.is_in_dependency_dir(path) {
                    return None; // Let DependenciesClassifier handle it
                }
                return Some(CollectionTier::Workspace);
            }
        }
        None
    }
}


// ============================================================================
// Helper Functions
// ============================================================================


/// Detect file type category
pub fn detect_file_type(path: &Path) -> String {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" | "c" | "cpp" | "go" | "py" | "js" | "ts" | "java" => "source_code".to_string(),
        "h" | "hpp" | "hxx" => "header".to_string(),
        "md" | "rst" | "txt" | "org" => "documentation".to_string(),
        "json" | "yaml" | "toml" | "xml" | "ini" | "conf" => "configuration".to_string(),
        "html" | "css" | "scss" => "web".to_string(),
        "sql" => "database".to_string(),
        "sh" | "bash" | "zsh" | "fish" => "script".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Find project root by looking for markers (.git, Cargo.toml, package.json, etc.)
pub fn find_project_root(path: &Path) -> Option<PathBuf> {
    let markers = [
        ".git",
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "pyproject.toml",
        "setup.py",
        "Makefile",
        "CMakeLists.txt",
    ];

    let mut current = path;

    while let Some(parent) = current.parent() {
        for marker in &markers {
            if parent.join(marker).exists() {
                return Some(parent.to_path_buf());
            }
        }
        current = parent;
    }

    None
}

/// Detect MIME type using mime_guess
pub fn detect_mime_type(path: &Path) -> Option<String> {
    mime_guess::from_path(path)
        .first()
        .map(|mime| mime.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> IndexingConfig {
        IndexingConfig {
            workspace_paths: vec![PathBuf::from("/home/user/projects")],
            personal_paths: vec![PathBuf::from("/home/user/Documents")],
            system_paths: vec![PathBuf::from("/usr/share/doc")],
            watch_enabled: true,
            chunk_size: 512,
            filters: IndexingFilters::default(),
            enable_qa_metadata: false,
        }
    }

    #[tokio::test]
    async fn test_classify_system_path() {
        let classifier = PathClassifier::new(&test_config());
        let path = PathBuf::from("/usr/share/man/man1/ls.1");

        let result = classifier.classify(&path).await.unwrap();
        assert_eq!(result.tier, CollectionTier::System);
    }

    #[tokio::test]
    async fn test_classify_personal_path() {
        let classifier = PathClassifier::new(&test_config());
        let path = PathBuf::from("/home/user/Documents/notes.md");

        let result = classifier.classify(&path).await.unwrap();
        assert_eq!(result.tier, CollectionTier::Personal);
    }

    #[tokio::test]
    async fn test_classify_workspace_path() {
        let classifier = PathClassifier::new(&test_config());
        let path = PathBuf::from("/home/user/projects/myapp/src/main.rs");

        let result = classifier.classify(&path).await.unwrap();
        assert_eq!(result.tier, CollectionTier::Workspace);
    }


    #[tokio::test]
    async fn test_detect_language() {
        assert_eq!(detect_languages(Path::new("main.rs")).await, [(Language::Rust,100f32)]);
        assert_eq!(detect_languages(Path::new("app.py")).await,[(Language::Python,100f32)]);
        assert_eq!(detect_languages(Path::new("script.js")).await, [(Language::JavaScript,100f32)]);
        assert_eq!(detect_languages(Path::new("unknown.xyz")).await, [(Language::Unknown, 100f32)]);
    }

    #[test]
    fn test_detect_file_type() {
        assert_eq!(detect_file_type(Path::new("main.rs")), "source_code");
        assert_eq!(detect_file_type(Path::new("README.md")), "documentation");
        assert_eq!(detect_file_type(Path::new("config.toml")), "configuration");
    }

    #[test]
    fn test_detect_mime_type() {
        assert!(detect_mime_type(Path::new("file.txt")).is_some());
        assert!(detect_mime_type(Path::new("image.png")).is_some());
        assert!(detect_mime_type(Path::new("video.mp4")).is_some());
    }
}
