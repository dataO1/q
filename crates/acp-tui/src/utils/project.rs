//! Project detection and analysis utilities

use crate::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Language distribution in a project
pub type LanguageDistribution = Vec<(String, f32)>;

/// Project scope information
#[derive(Debug, Clone)]
pub struct ProjectScope {
    /// Root directory of the project
    pub root: PathBuf,
    
    /// Current file being worked on (if any)
    pub current_file: Option<PathBuf>,
    
    /// Distribution of programming languages
    pub language_distribution: LanguageDistribution,
    
    /// Key project files
    pub key_files: Vec<ProjectFile>,
    
    /// Project type (detected from structure)
    pub project_type: ProjectType,
}

/// Information about a key project file
#[derive(Debug, Clone)]
pub struct ProjectFile {
    /// Relative path from project root
    pub path: PathBuf,
    
    /// Purpose of the file
    pub purpose: FilePurpose,
    
    /// File size in bytes
    pub size: u64,
    
    /// Last modified time
    pub last_modified: std::time::SystemTime,
}

/// Purpose of a project file
#[derive(Debug, Clone, PartialEq)]
pub enum FilePurpose {
    /// Build configuration (Cargo.toml, package.json, etc.)
    BuildConfig,
    
    /// Main entry point
    EntryPoint,
    
    /// Documentation
    Documentation,
    
    /// Configuration file
    Configuration,
    
    /// Test file
    Test,
    
    /// Source code
    Source,
    
    /// Unknown/other purpose
    Other,
}

/// Type of project detected
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    /// Rust project (has Cargo.toml)
    Rust,
    
    /// Node.js project (has package.json)
    NodeJs,
    
    /// Python project (has requirements.txt, setup.py, pyproject.toml)
    Python,
    
    /// Go project (has go.mod)
    Go,
    
    /// Java project (has pom.xml, build.gradle)
    Java,
    
    /// Generic project
    Generic,
}

impl ProjectScope {
    /// Detect project scope from a directory
    pub fn detect(directory: impl AsRef<Path>) -> Result<Self> {
        let root = directory.as_ref().canonicalize()
            .map_err(|e| Error::Io(e))?;
            
        let project_type = detect_project_type(&root)?;
        let language_distribution = analyze_language_distribution(&root)?;
        let key_files = find_key_files(&root, &project_type)?;
        
        Ok(Self {
            root,
            current_file: None,
            language_distribution,
            key_files,
            project_type,
        })
    }
    
    /// Set the current file being worked on
    pub fn with_current_file(mut self, file: impl AsRef<Path>) -> Self {
        self.current_file = Some(file.as_ref().to_path_buf());
        self
    }
    
    /// Get a summary of the project
    pub fn summary(&self) -> String {
        let lang_summary = if self.language_distribution.is_empty() {
            "unknown languages".to_string()
        } else {
            let top_lang = &self.language_distribution[0];
            if self.language_distribution.len() == 1 {
                format!("{} project", top_lang.0)
            } else {
                format!("{} project ({}% {})", 
                    self.project_type.as_str(),
                    (top_lang.1 * 100.0) as u32,
                    top_lang.0
                )
            }
        };
        
        format!("{} at {}", lang_summary, self.root.display())
    }
    
    /// Check if this is likely a workspace/multi-project directory
    pub fn is_workspace(&self) -> bool {
        match self.project_type {
            ProjectType::Rust => {
                // Check for workspace Cargo.toml
                if let Ok(content) = std::fs::read_to_string(self.root.join("Cargo.toml")) {
                    return content.contains("[workspace]");
                }
                false
            }
            ProjectType::NodeJs => {
                // Check for workspace package.json or lerna.json
                self.root.join("lerna.json").exists() ||
                self.root.join("workspaces").exists()
            }
            _ => false,
        }
    }
}

impl ProjectType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectType::Rust => "Rust",
            ProjectType::NodeJs => "Node.js",
            ProjectType::Python => "Python",
            ProjectType::Go => "Go",
            ProjectType::Java => "Java",
            ProjectType::Generic => "Generic",
        }
    }
}

/// Detect the project type from directory contents
fn detect_project_type(root: &Path) -> Result<ProjectType> {
    // Check for specific project markers
    if root.join("Cargo.toml").exists() {
        return Ok(ProjectType::Rust);
    }
    
    if root.join("package.json").exists() {
        return Ok(ProjectType::NodeJs);
    }
    
    if root.join("go.mod").exists() {
        return Ok(ProjectType::Go);
    }
    
    if root.join("pom.xml").exists() || root.join("build.gradle").exists() {
        return Ok(ProjectType::Java);
    }
    
    if root.join("requirements.txt").exists() || 
       root.join("setup.py").exists() || 
       root.join("pyproject.toml").exists() {
        return Ok(ProjectType::Python);
    }
    
    Ok(ProjectType::Generic)
}

/// Analyze language distribution in the project
fn analyze_language_distribution(root: &Path) -> Result<LanguageDistribution> {
    let mut language_counts: HashMap<String, usize> = HashMap::new();
    let mut total_files = 0;
    
    analyze_directory(root, &mut language_counts, &mut total_files, 0)?;
    
    if total_files == 0 {
        return Ok(vec![("Unknown".to_string(), 1.0)]);
    }
    
    let mut distribution: Vec<(String, f32)> = language_counts
        .into_iter()
        .map(|(lang, count)| (lang, count as f32 / total_files as f32))
        .collect();
    
    // Sort by percentage (descending)
    distribution.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    // Keep only languages with > 1% representation
    distribution.retain(|(_, pct)| *pct > 0.01);
    
    if distribution.is_empty() {
        distribution.push(("Unknown".to_string(), 1.0));
    }
    
    Ok(distribution)
}

/// Recursively analyze a directory for language files
fn analyze_directory(
    dir: &Path,
    language_counts: &mut HashMap<String, usize>,
    total_files: &mut usize,
    depth: usize,
) -> Result<()> {
    // Limit recursion depth to prevent excessive scanning
    if depth > 10 {
        return Ok(());
    }
    
    let entries = std::fs::read_dir(dir).map_err(Error::Io)?;
    
    for entry in entries {
        let entry = entry.map_err(Error::Io)?;
        let path = entry.path();
        
        // Skip hidden files and common ignore patterns
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || 
               name == "target" || 
               name == "node_modules" || 
               name == "__pycache__" ||
               name == "dist" ||
               name == "build" {
                continue;
            }
        }
        
        if path.is_dir() {
            analyze_directory(&path, language_counts, total_files, depth + 1)?;
        } else if path.is_file() {
            *total_files += 1;
            
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let language = extension_to_language(ext);
                *language_counts.entry(language.to_string()).or_insert(0) += 1;
            }
        }
    }
    
    Ok(())
}

/// Map file extension to programming language
fn extension_to_language(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "rs" => "Rust",
        "py" => "Python",
        "js" | "mjs" => "JavaScript",
        "ts" => "TypeScript",
        "java" => "Java",
        "c" => "C",
        "cpp" | "cc" | "cxx" | "hpp" => "C++",
        "go" => "Go",
        "hs" => "Haskell",
        "lua" => "Lua",
        "yml" | "yaml" => "YAML",
        "sh" | "bash" => "Shell",
        "html" | "htm" => "HTML",
        "json" => "JSON",
        "rb" => "Ruby",
        "md" | "markdown" => "Markdown",
        "toml" => "TOML",
        "xml" => "XML",
        "css" => "CSS",
        "scss" | "sass" => "Sass",
        "php" => "PHP",
        "swift" => "Swift",
        "kt" => "Kotlin",
        "scala" => "Scala",
        "clj" | "cljs" => "Clojure",
        "ex" | "exs" => "Elixir",
        "elm" => "Elm",
        "dart" => "Dart",
        "vue" => "Vue",
        "svelte" => "Svelte",
        _ => "Other",
    }
}

/// Find key files in the project
fn find_key_files(root: &Path, project_type: &ProjectType) -> Result<Vec<ProjectFile>> {
    let mut key_files = Vec::new();
    
    // Always look for README files
    for readme in &["README.md", "README.txt", "README.rst", "README"] {
        if let Some(file) = try_add_file(root, readme, FilePurpose::Documentation) {
            key_files.push(file);
            break; // Only add the first README found
        }
    }
    
    // Project-specific files
    match project_type {
        ProjectType::Rust => {
            if let Some(file) = try_add_file(root, "Cargo.toml", FilePurpose::BuildConfig) {
                key_files.push(file);
            }
            if let Some(file) = try_add_file(root, "src/main.rs", FilePurpose::EntryPoint) {
                key_files.push(file);
            }
            if let Some(file) = try_add_file(root, "src/lib.rs", FilePurpose::EntryPoint) {
                key_files.push(file);
            }
        }
        ProjectType::NodeJs => {
            if let Some(file) = try_add_file(root, "package.json", FilePurpose::BuildConfig) {
                key_files.push(file);
            }
            if let Some(file) = try_add_file(root, "index.js", FilePurpose::EntryPoint) {
                key_files.push(file);
            }
            if let Some(file) = try_add_file(root, "src/index.js", FilePurpose::EntryPoint) {
                key_files.push(file);
            }
        }
        ProjectType::Python => {
            if let Some(file) = try_add_file(root, "requirements.txt", FilePurpose::BuildConfig) {
                key_files.push(file);
            }
            if let Some(file) = try_add_file(root, "setup.py", FilePurpose::BuildConfig) {
                key_files.push(file);
            }
            if let Some(file) = try_add_file(root, "main.py", FilePurpose::EntryPoint) {
                key_files.push(file);
            }
        }
        _ => {}
    }
    
    Ok(key_files)
}

/// Try to add a file to the key files list if it exists
fn try_add_file(root: &Path, relative_path: &str, purpose: FilePurpose) -> Option<ProjectFile> {
    let path = root.join(relative_path);
    if let Ok(metadata) = path.metadata() {
        if metadata.is_file() {
            return Some(ProjectFile {
                path: PathBuf::from(relative_path),
                purpose,
                size: metadata.len(),
                last_modified: metadata.modified().unwrap_or(std::time::UNIX_EPOCH),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extension_to_language() {
        assert_eq!(extension_to_language("rs"), "Rust");
        assert_eq!(extension_to_language("py"), "Python");
        assert_eq!(extension_to_language("JS"), "JavaScript");
        assert_eq!(extension_to_language("unknown"), "Other");
    }
    
    #[test]
    fn test_project_type_as_str() {
        assert_eq!(ProjectType::Rust.as_str(), "Rust");
        assert_eq!(ProjectType::NodeJs.as_str(), "Node.js");
        assert_eq!(ProjectType::Generic.as_str(), "Generic");
    }
}