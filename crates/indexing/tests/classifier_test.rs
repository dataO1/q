use ai_agent_indexing::classifier::*;
use ai_agent_common::config::*;
use ai_agent_common::types::CollectionTier;
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs;

/// Helper to create test configuration
fn test_config() -> IndexingConfig {
    let temp_workspace = TempDir::new().unwrap();
    let temp_personal = TempDir::new().unwrap();

    IndexingConfig {
        workspace_paths: vec![temp_workspace.path().to_path_buf()],
        personal_paths: vec![temp_personal.path().to_path_buf()],
        system_paths: vec![PathBuf::from("/usr/share/doc")],
        watch_enabled: true,
        chunk_size: 512,
        filters: IndexingFilters::default(),
    }
}

/// Helper to create realistic test paths
fn create_test_paths(base: &TempDir) -> Vec<PathBuf> {
    let paths = vec![
        "src/main.rs",
        "src/lib.rs",
        "tests/test.py",
        "README.md",
        "Cargo.toml",
        "package.json",
        ".git/config",
        "node_modules/react/index.js",
        "target/debug/deps/lib.rlib",
    ];

    let mut created = Vec::new();
    for path in paths {
        let full_path = base.path().join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&full_path, "test content").ok();
        created.push(full_path);
    }
    created
}

// ============================================================================
// Tier Classification Tests
// ============================================================================

#[tokio::test]
async fn test_classify_system_paths() {
    let config = test_config();
    let classifier = PathClassifier::new(&config);

    let system_paths = vec![
        "/usr/share/man/man1/ls.1",
        "/usr/share/doc/git/README",
        "/usr/include/stdio.h",
        "/etc/passwd",
    ];

    for path_str in system_paths {
        let path = PathBuf::from(path_str);
        let result = classifier.classify(&path).await.unwrap();
        assert_eq!(
            result.tier,
            CollectionTier::System,
            "Failed to classify {} as System",
            path_str
        );
    }
}

#[tokio::test]
async fn test_classify_personal_paths() {
    let temp_personal = TempDir::new().unwrap();
    let config = IndexingConfig {
        personal_paths: vec![temp_personal.path().to_path_buf()],
        workspace_paths: vec![],
        system_paths: vec![],
        watch_enabled: true,
        chunk_size: 512,
        filters: IndexingFilters::default(),
    };

    let classifier = PathClassifier::new(&config);

    let personal_file = temp_personal.path().join("notes.md");
    fs::write(&personal_file, "personal notes").unwrap();

    let result = classifier.classify(&personal_file).await.unwrap();
    assert_eq!(result.tier, CollectionTier::Personal);
}

#[tokio::test]
async fn test_classify_workspace_paths() {
    let temp_workspace = TempDir::new().unwrap();
    let config = IndexingConfig {
        workspace_paths: vec![temp_workspace.path().to_path_buf()],
        personal_paths: vec![],
        system_paths: vec![],
        watch_enabled: true,
        chunk_size: 512,
        filters: IndexingFilters::default(),
    };

    let classifier = PathClassifier::new(&config);

    let workspace_file = temp_workspace.path().join("src/main.rs");
    fs::create_dir_all(temp_workspace.path().join("src")).unwrap();
    fs::write(&workspace_file, "fn main() {}").unwrap();

    let result = classifier.classify(&workspace_file).await.unwrap();
    assert_eq!(result.tier, CollectionTier::Workspace);
}

#[tokio::test]
async fn test_classify_dependencies() {
    let temp_workspace = TempDir::new().unwrap();
    let config = IndexingConfig {
        workspace_paths: vec![temp_workspace.path().to_path_buf()],
        personal_paths: vec![],
        system_paths: vec![],
        watch_enabled: true,
        chunk_size: 512,
        filters: IndexingFilters::default(),
    };

    let classifier = PathClassifier::new(&config);

    // Test node_modules
    let node_module = temp_workspace.path().join("node_modules/react/index.js");
    fs::create_dir_all(node_module.parent().unwrap()).unwrap();
    fs::write(&node_module, "module.exports = {}").unwrap();

    let result = classifier.classify(&node_module).await.unwrap();
    assert_eq!(result.tier, CollectionTier::Dependencies);

    // Test vendor
    let vendor_file = temp_workspace.path().join("vendor/lib/code.rb");
    fs::create_dir_all(vendor_file.parent().unwrap()).unwrap();
    fs::write(&vendor_file, "class Lib; end").unwrap();

    let result = classifier.classify(&vendor_file).await.unwrap();
    assert_eq!(result.tier, CollectionTier::Dependencies);
}

#[tokio::test]
async fn test_priority_ordering() {
    // System paths should take priority over personal/workspace
    let temp_workspace = TempDir::new().unwrap();
    let system_in_workspace = temp_workspace.path().join("usr/share/man/test.1");

    fs::create_dir_all(system_in_workspace.parent().unwrap()).unwrap();
    fs::write(&system_in_workspace, "man page").unwrap();

    let config = IndexingConfig {
        workspace_paths: vec![temp_workspace.path().to_path_buf()],
        personal_paths: vec![],
        system_paths: vec![PathBuf::from("/usr/share/man")],
        watch_enabled: true,
        chunk_size: 512,
        filters: IndexingFilters::default(),
    };

    let classifier = PathClassifier::new(&config);

    // Even though it's under workspace, /usr/share/man should match first
    let system_path = PathBuf::from("/usr/share/man/man1/ls.1");
    let result = classifier.classify(&system_path).await.unwrap();
    assert_eq!(result.tier, CollectionTier::System);
}

// ============================================================================
// Language Detection Tests
// ============================================================================

#[test]
fn test_language_detection_rust() {
    assert_eq!(detect_language(&PathBuf::from("main.rs")), Some("rust".to_string()));
    assert_eq!(detect_language(&PathBuf::from("lib.rs")), Some("rust".to_string()));
}

#[test]
fn test_language_detection_python() {
    assert_eq!(detect_language(&PathBuf::from("script.py")), Some("python".to_string()));
    assert_eq!(detect_language(&PathBuf::from("app.pyw")), Some("python".to_string()));
}

#[test]
fn test_language_detection_javascript() {
    assert_eq!(detect_language(&PathBuf::from("app.js")), Some("javascript".to_string()));
    assert_eq!(detect_language(&PathBuf::from("module.mjs")), Some("javascript".to_string()));
    assert_eq!(detect_language(&PathBuf::from("config.cjs")), Some("javascript".to_string()));
}

#[test]
fn test_language_detection_typescript() {
    assert_eq!(detect_language(&PathBuf::from("app.ts")), Some("typescript".to_string()));
    assert_eq!(detect_language(&PathBuf::from("component.tsx")), Some("typescript".to_string()));
}

#[test]
fn test_language_detection_web() {
    assert_eq!(detect_language(&PathBuf::from("index.html")), Some("html".to_string()));
    assert_eq!(detect_language(&PathBuf::from("style.css")), Some("css".to_string()));
    assert_eq!(detect_language(&PathBuf::from("style.scss")), Some("css".to_string()));
}

#[test]
fn test_language_detection_config() {
    assert_eq!(detect_language(&PathBuf::from("config.json")), Some("json".to_string()));
    assert_eq!(detect_language(&PathBuf::from("config.yaml")), Some("yaml".to_string()));
    assert_eq!(detect_language(&PathBuf::from("Cargo.toml")), Some("toml".to_string()));
}

#[test]
fn test_language_detection_docs() {
    assert_eq!(detect_language(&PathBuf::from("README.md")), Some("markdown".to_string()));
    assert_eq!(detect_language(&PathBuf::from("doc.rst")), Some("restructuredtext".to_string()));
}

#[test]
fn test_language_detection_unknown() {
    assert_eq!(detect_language(&PathBuf::from("unknown.xyz")), None);
    assert_eq!(detect_language(&PathBuf::from("no_extension")), None);
}

// ============================================================================
// File Type Detection Tests
// ============================================================================

#[test]
fn test_file_type_source_code() {
    assert_eq!(detect_file_type(&PathBuf::from("main.rs")), "source_code");
    assert_eq!(detect_file_type(&PathBuf::from("app.py")), "source_code");
    assert_eq!(detect_file_type(&PathBuf::from("script.js")), "source_code");
}

#[test]
fn test_file_type_header() {
    assert_eq!(detect_file_type(&PathBuf::from("header.h")), "header");
    assert_eq!(detect_file_type(&PathBuf::from("class.hpp")), "header");
}

#[test]
fn test_file_type_documentation() {
    assert_eq!(detect_file_type(&PathBuf::from("README.md")), "documentation");
    assert_eq!(detect_file_type(&PathBuf::from("doc.rst")), "documentation");
    assert_eq!(detect_file_type(&PathBuf::from("notes.txt")), "documentation");
}

#[test]
fn test_file_type_configuration() {
    assert_eq!(detect_file_type(&PathBuf::from("config.json")), "configuration");
    assert_eq!(detect_file_type(&PathBuf::from("settings.yaml")), "configuration");
    assert_eq!(detect_file_type(&PathBuf::from("Cargo.toml")), "configuration");
}

#[test]
fn test_file_type_web() {
    assert_eq!(detect_file_type(&PathBuf::from("index.html")), "web");
    assert_eq!(detect_file_type(&PathBuf::from("style.css")), "web");
}

#[test]
fn test_file_type_script() {
    assert_eq!(detect_file_type(&PathBuf::from("build.sh")), "script");
    assert_eq!(detect_file_type(&PathBuf::from("setup.bash")), "script");
}

// ============================================================================
// Project Root Detection Tests
// ============================================================================

#[test]
fn test_find_project_root_git() {
    let temp = TempDir::new().unwrap();
    let git_dir = temp.path().join(".git");
    fs::create_dir(&git_dir).unwrap();

    let nested_file = temp.path().join("src/main.rs");
    fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
    fs::write(&nested_file, "fn main() {}").unwrap();

    let root = find_project_root(&nested_file);
    assert_eq!(root, Some(temp.path().to_path_buf()));
}

#[test]
fn test_find_project_root_cargo() {
    let temp = TempDir::new().unwrap();
    let cargo_toml = temp.path().join("Cargo.toml");
    fs::write(&cargo_toml, "[package]\nname = \"test\"").unwrap();

    let nested_file = temp.path().join("src/lib.rs");
    fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
    fs::write(&nested_file, "pub fn test() {}").unwrap();

    let root = find_project_root(&nested_file);
    assert_eq!(root, Some(temp.path().to_path_buf()));
}

#[test]
fn test_find_project_root_package_json() {
    let temp = TempDir::new().unwrap();
    let package_json = temp.path().join("package.json");
    fs::write(&package_json, "{}").unwrap();

    let nested_file = temp.path().join("src/index.js");
    fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
    fs::write(&nested_file, "console.log('test')").unwrap();

    let root = find_project_root(&nested_file);
    assert_eq!(root, Some(temp.path().to_path_buf()));
}

#[test]
fn test_find_project_root_no_marker() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("random.txt");
    fs::write(&file, "content").unwrap();

    let root = find_project_root(&file);
    assert_eq!(root, None);
}

// ============================================================================
// MIME Type Detection Tests
// ============================================================================

#[test]
fn test_mime_type_text() {
    let mime = detect_mime_type(&PathBuf::from("file.txt"));
    assert!(mime.is_some());
    assert!(mime.unwrap().contains("text"));
}

#[test]
fn test_mime_type_json() {
    let mime = detect_mime_type(&PathBuf::from("data.json"));
    assert!(mime.is_some());
    assert!(mime.unwrap().contains("json"));
}

#[test]
fn test_mime_type_image() {
    let mime = detect_mime_type(&PathBuf::from("image.png"));
    assert!(mime.is_some());
    assert!(mime.unwrap().contains("image"));
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_full_classification_workflow() {
    let temp_workspace = TempDir::new().unwrap();

    // Create a realistic project structure
    let paths = create_test_paths(&temp_workspace);

    let config = IndexingConfig {
        workspace_paths: vec![temp_workspace.path().to_path_buf()],
        personal_paths: vec![],
        system_paths: vec![],
        watch_enabled: true,
        chunk_size: 512,
        filters: IndexingFilters::default(),
    };

    let classifier = PathClassifier::new(&config);

    // Test main.rs
    let main_rs = &paths[0]; // src/main.rs
    let result = classifier.classify(main_rs).await.unwrap();
    assert_eq!(result.tier, CollectionTier::Workspace);
    assert_eq!(result.language, Some("rust".to_string()));
    assert_eq!(result.file_type, "source_code");
    assert!(result.project_root.is_some());

    // Test dependency
    let node_module = &paths[7]; // node_modules/react/index.js
    let result = classifier.classify(node_module).await.unwrap();
    assert_eq!(result.tier, CollectionTier::Dependencies);
    assert_eq!(result.language, Some("javascript".to_string()));
}

#[tokio::test]
async fn test_classification_result_completeness() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("src/main.rs");

    fs::create_dir_all(test_file.parent().unwrap()).unwrap();
    fs::write(&test_file, "fn main() {}").unwrap();

    // Add Cargo.toml for project root detection
    fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

    let config = IndexingConfig {
        workspace_paths: vec![temp.path().to_path_buf()],
        personal_paths: vec![],
        system_paths: vec![],
        watch_enabled: true,
        chunk_size: 512,
        filters: IndexingFilters::default(),
    };

    let classifier = PathClassifier::new(&config);
    let result = classifier.classify(&test_file).await.unwrap();

    // Verify all fields are populated
    assert_eq!(result.tier, CollectionTier::Workspace);
    assert!(result.language.is_some());
    assert_eq!(result.language.unwrap(), "rust");
    assert_eq!(result.file_type, "source_code");
    assert!(result.project_root.is_some());
    assert!(result.mime_type.is_some());
}

