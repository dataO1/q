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

pub fn determine_chunk_strategy(path: &Path, config: &Config) -> ChunkStrategy {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let ext_lower = ext.to_lowercase();
            // Fall back to hardcoded mappings
            match ext_lower.as_str() {
                "rs" => ChunkStrategy::Code { language: "rust" },
                "py" => ChunkStrategy::Code { language: "python" },
                "js" => ChunkStrategy::Code { language: "javascript" },
                "ts" => ChunkStrategy::Code { language: "typescript" },
                "go" => ChunkStrategy::Code { language: "go" },
                "c" | "h" => ChunkStrategy::Code { language: "c" },
                "cpp" | "cc" | "cxx" | "c++" => ChunkStrategy::Code { language: "cpp" },
                "java" => ChunkStrategy::Code { language: "java" },
                "rb" => ChunkStrategy::Code { language: "ruby" },
                "nix" => ChunkStrategy::Code { language: "nix" },
                "html" => ChunkStrategy::Code { language: "html" },
                "php" => ChunkStrategy::Code { language: "php" },
                "md" => ChunkStrategy::Markdown,
                _ => ChunkStrategy::PlainText,
            }
        }
        None => ChunkStrategy::PlainText,
    }
}
