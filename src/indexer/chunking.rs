use crate::Config;
use std::path::Path;
use std::fmt::{Display, Formatter, Result};

pub enum ChunkStrategy {
    Code { language: &'static str },
    Markdown,
    PlainText,
}

impl Display for ChunkStrategy {
    fn fmt(&self, f:&mut Formatter) -> Result {
        match self{
            ChunkStrategy::PlainText => write!(f, "Plaintext"),
            ChunkStrategy::Markdown => write!(f, "Markdown"),
            ChunkStrategy::Code{language} => write!(f,"Treesitter {}",*language),
        }
    }
}

pub fn determine_chunk_strategy(path: &Path, _config: &Config) -> ChunkStrategy {;
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
