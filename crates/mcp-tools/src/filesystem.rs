use async_trait::async_trait;

use crate::Tool;

pub struct FileSystemTool;

impl FileSystemTool {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for FileSystemTool {
    fn name(&self) -> &str {
        "filesystem"
    }

    fn description(&self) -> &str {
        "File system operations (read, write, list)"
    }

    async fn call(&self, args: serde_json::Value) -> anyhow::Result<String> {
        todo!("Implement filesystem tool commands")
    }
}
