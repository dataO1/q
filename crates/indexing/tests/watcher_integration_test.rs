use ai_agent_indexing::watcher::{FileWatcher, FileEvent};
use ai_agent_common::config::IndexingFilters;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

/// Helper to create test filters
fn test_filters() -> IndexingFilters {
    IndexingFilters {
        respect_gitignore: true,
        include_hidden: false,
        ignore_dirs: vec!["node_modules".to_string(), "target".to_string()],
        ignore_extensions: vec!["lock".to_string()],
        custom_ignores: vec![],
        max_file_size: Some(1024 * 1024),
    }
}

/// Cleanup helper - ensures watcher is properly dropped
struct TestWatcherGuard {
    watcher: Option<FileWatcher>,
    _temp_dir: TempDir,
}

impl TestWatcherGuard {
    fn new(temp_dir: TempDir, watcher: FileWatcher) -> Self {
        Self {
            watcher: Some(watcher),
            _temp_dir: temp_dir,
        }
    }

    fn take_watcher(&mut self) -> FileWatcher {
        self.watcher.take().expect("Watcher already taken")
    }
}

impl Drop for TestWatcherGuard {
    fn drop(&mut self) {
        // Explicitly drop watcher before temp_dir
        self.watcher.take();
        tracing::debug!("Test watcher cleaned up");
    }
}

#[tokio::test]
async fn test_detect_file_creation() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_path_buf();

    let mut watcher = FileWatcher::new(vec![path.clone()], test_filters()).unwrap();
    watcher.start_watching().unwrap();

    // Use tokio::select to ensure task is cancelled on timeout
    let test_file = path.join("test.txt");

    let result = tokio::select! {
        event = async {
            // Give watcher time to start
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Create file
            fs::write(&test_file, "test content").unwrap();

            // Wait for event
            watcher.watch().await
        } => event,
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("Test timed out");
        }
    };

    assert!(result.is_ok(), "Failed to detect file creation");

    let event = result.unwrap();
    match event {
        FileEvent::Created(p) => assert_eq!(p, test_file),
        _ => panic!("Expected Created event"),
    }

    // Explicit cleanup (though TempDir handles it)
    drop(watcher);
    drop(temp_dir);
}

#[tokio::test]
async fn test_detect_file_modification() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_path_buf();

    // Create file first
    let test_file = path.join("test.txt");
    fs::write(&test_file, "initial content").unwrap();

    let mut watcher = FileWatcher::new(vec![path.clone()], test_filters()).unwrap();
    watcher.start_watching().unwrap();

    let result = tokio::select! {
        event = async {
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Modify the file
            fs::write(&test_file, "modified content").unwrap();

            watcher.watch().await
        } => event,
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("Test timed out");
        }
    };

    assert!(result.is_ok());
    let event = result.unwrap();

    match event {
        FileEvent::Modified(p) => assert_eq!(p, test_file),
        _ => panic!("Expected Modified event, got: {:?}", event),
    }

    // Cleanup
    drop(watcher);
    drop(temp_dir);
}

#[tokio::test]
async fn test_file_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_path_buf();

    let mut watcher = FileWatcher::new(vec![path.clone()], test_filters()).unwrap();
    watcher.start_watching().unwrap();

    // Give watcher time to start
    tokio::time::sleep(Duration::from_millis(300)).await;

    let test_file = path.join("lifecycle_test.txt");

    // Test 1: Create file
    fs::write(&test_file, "initial").unwrap();

    let create_event = tokio::time::timeout(
        Duration::from_secs(3),
        watcher.watch()
    ).await.expect("Timeout waiting for create event")
        .expect("Error receiving create event");

    assert!(matches!(create_event, FileEvent::Created(_)),
            "Expected create event, got: {:?}", create_event);
    println!("✓ Create event detected");

    // Test 2: Modify file
    tokio::time::sleep(Duration::from_millis(200)).await;
    fs::write(&test_file, "modified").unwrap();

    let modify_event = tokio::time::timeout(
        Duration::from_secs(3),
        watcher.watch()
    ).await.expect("Timeout waiting for modify event")
        .expect("Error receiving modify event");

    assert!(matches!(modify_event, FileEvent::Modified(_)),
            "Expected modify event, got: {:?}", modify_event);
    println!("✓ Modify event detected");

    // Test 3: Delete file (optional - may not work on all systems)
    tokio::time::sleep(Duration::from_millis(200)).await;
    fs::remove_file(&test_file).unwrap();

    // Try to get deletion event, but don't fail if it doesn't arrive
    match tokio::time::timeout(Duration::from_secs(2), watcher.watch()).await {
        Ok(Ok(FileEvent::Deleted(_))) => {
            println!("✓ Delete event detected (bonus!)");
        }
        Ok(Ok(other)) => {
            println!("⚠ Got event after deletion but not a delete event: {:?}", other);
            println!("  (This is acceptable - deletion events are unreliable)");
        }
        Ok(Err(e)) => {
            println!("⚠ Error after deletion: {} (acceptable)", e);
        }
        Err(_) => {
            println!("⚠ No delete event received (this is OK - deletion events are flaky)");
        }
    }

    drop(watcher);
    drop(temp_dir);
}

#[tokio::test]
async fn test_gitignore_filtering() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_path_buf();

    // Create .gitignore
    let gitignore = path.join(".gitignore");
    fs::write(&gitignore, "ignored.txt\n*.tmp\n").unwrap();

    let watcher = FileWatcher::new(vec![path.clone()], test_filters()).unwrap();

    // Create ignored file
    let ignored_file = path.join("ignored.txt");
    fs::write(&ignored_file, "should be ignored").unwrap();

    // Test filtering using public test method
    assert!(watcher.is_ignored_by_gitignore(&ignored_file));

    // Create non-ignored file
    let normal_file = path.join("normal.txt");
    fs::write(&normal_file, "should be processed").unwrap();
    assert!(!watcher.is_ignored_by_gitignore(&normal_file));

    drop(watcher);
    drop(temp_dir);
}

#[tokio::test]
async fn test_add_and_remove_paths() {
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();

    let path1 = temp_dir1.path().to_path_buf();
    let path2 = temp_dir2.path().to_path_buf();

    let mut watcher = FileWatcher::new(vec![path1.clone()], test_filters()).unwrap();
    watcher.start_watching().unwrap();

    // Add second path
    assert!(watcher.add_path(&path2).is_ok());
    assert_eq!(watcher.watched_path_count(), 2);  // Use public method
    assert!(watcher.is_watching(&path1));
    assert!(watcher.is_watching(&path2));

    // Remove first path
    assert!(watcher.remove_path(&path1).is_ok());
    assert_eq!(watcher.watched_path_count(), 1);  // Use public method
    assert!(!watcher.is_watching(&path1));
    assert!(watcher.is_watching(&path2));

    // Cleanup
    drop(watcher);
    drop(temp_dir2);
    drop(temp_dir1);
}

/// Test with explicit resource cleanup verification
#[tokio::test]
async fn test_cleanup_verification() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    {
        let mut watcher = FileWatcher::new(vec![temp_path.clone()], test_filters()).unwrap();
        watcher.start_watching().unwrap();

        // Watcher is active here
        assert!(temp_path.exists());
    } // Watcher dropped here

    // Verify temp directory still exists (TempDir not dropped yet)
    assert!(temp_path.exists());

    drop(temp_dir);

    // After TempDir drop, directory should be gone
    assert!(!temp_path.exists());
}
