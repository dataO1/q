use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Create a test directory structure with various files
pub fn create_test_structure() -> TempDir {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Regular files
    fs::write(root.join("main.rs"), "fn main() {}").unwrap();
    fs::write(root.join("lib.rs"), "pub mod test;").unwrap();

    // Hidden files
    fs::write(root.join(".hidden"), "secret").unwrap();
    fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();

    // Create subdirectories
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/test.rs"), "test").unwrap();

    fs::create_dir(root.join("target")).unwrap();
    fs::write(root.join("target/debug.log"), "logs").unwrap();

    fs::create_dir(root.join("node_modules")).unwrap();
    fs::write(root.join("node_modules/pkg.json"), "{}").unwrap();

    // Files with various extensions
    fs::write(root.join("Cargo.lock"), "lock").unwrap();
    fs::write(root.join("Cargo.toml"), "[package]").unwrap();

    temp
}

/// Create a gitignore file with patterns
pub fn create_gitignore(path: &Path, patterns: &[&str]) {
    let content = patterns.join("\n");
    fs::write(path.join(".gitignore"), content).unwrap();
}
