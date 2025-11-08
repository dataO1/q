use crate::Config;
use std::path::Path;

pub enum ChunkStrategy {
    Code { language: &'static str },
    Markdown,
    PlainText,
}

pub fn determine_chunk_strategy(path: &Path, _config: &Config) -> ChunkStrategy {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => ChunkStrategy::Code { language: "rust" },
        Some("py") => ChunkStrategy::Code { language: "python" },
        Some("js") => ChunkStrategy::Code { language: "javascript" },
        Some("ts") => ChunkStrategy::Code { language: "typescript" },
        Some("go") => ChunkStrategy::Code { language: "go" },
        Some("java") => ChunkStrategy::Code { language: "java" },
        Some("cpp") | Some("cc") | Some("cxx") => ChunkStrategy::Code { language: "cpp" },
        Some("c") | Some("h") => ChunkStrategy::Code { language: "c" },
        Some("md") => ChunkStrategy::Markdown,
        _ => ChunkStrategy::PlainText,
    }
}
