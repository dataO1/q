use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Asynchronously finds the git repository root starting from the provided path
pub async fn find_git_root(start: &Path) -> Result<Option<PathBuf>> {
    let mut current = Some(start);

    while let Some(path) = current {
        let git_path = path.join(".git");
        if fs::metadata(&git_path).await.is_ok() {
            return Ok(Some(path.to_path_buf()));
        }
        current = path.parent();
    }

    Ok(None) // No git root found
}
