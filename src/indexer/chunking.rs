use crate::Config;
use std::path::Path;

pub enum ChunkStrategy {
    Code { language: String },
    Markdown,
    PlainText,
}

pub fn determine_chunk_strategy(path: &Path, config: &Config) -> ChunkStrategy {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // Check if markdown
    if extension == "md" {
        return ChunkStrategy::Markdown;
    }

    // Check if we have a tree-sitter mapping
    if let Some(language) = config.get_language_for_extension(extension) {
        return ChunkStrategy::Code {
            language: language.to_string(),
        };
    }

    // Fallback to plain text
    ChunkStrategy::PlainText
}
