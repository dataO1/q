//! Chunk code using tree-sitter
use std::path::Path;

use anyhow::{Context as _, Result};
use async_trait::async_trait;
use derive_builder::Builder;

use swiftide::{indexing::{transformers::ChunkCode, IndexingStream, TextNode}, traits::ChunkerTransformer};
use swiftide_indexing::transformers::{ChunkMarkdown, ChunkText};
use swiftide_integrations::treesitter::SupportedLanguages;
use tracing::info;

const DEFAULT_MAX_CHAR_SIZE: usize = 2056;
/// The `ChunkCode` struct is responsible for chunking code into smaller pieces
/// based on the specified language and chunk size.
///
/// It uses tree-sitter under the hood, and tries to split the code into smaller, meaningful
/// chunks.
///
/// # Example
///
/// ```no_run
/// # use swiftide_integrations::treesitter::transformers::ChunkCode;
/// # use swiftide_integrations::treesitter::SupportedLanguages;
/// // Chunk rust code with a maximum chunk size of 1000 bytes.
/// ChunkCode::try_for_language_and_chunk_size(SupportedLanguages::Rust, 1000);
///
/// // Chunk python code with a minimum chunk size of 500 bytes and maximum chunk size of 2048.
/// // Smaller chunks than 500 bytes will be discarded.
/// ChunkCode::try_for_language_and_chunk_size(SupportedLanguages::Python, 500..2048);
/// ````
#[derive(Debug, Clone) ]
pub struct ChunkAdaptive {
    chunk_size_code:usize,
    chunk_size_markdown:usize,
    chunk_size_text:usize,
    concurrency: Option<usize>
}


impl Default for ChunkAdaptive {
    fn default() -> Self {
        Self{
            chunk_size_markdown:DEFAULT_MAX_CHAR_SIZE,
            chunk_size_text: DEFAULT_MAX_CHAR_SIZE,
            chunk_size_code: DEFAULT_MAX_CHAR_SIZE,
            concurrency: None
        }
    }

}

impl ChunkAdaptive {
    /// Tries to create a `ChunkCode` instance for a given programming language.
    ///
    /// # Parameters
    /// - `lang`: The programming language to be used for chunking. It should implement
    ///   `TryInto<SupportedLanguages>`.
    ///
    /// # Returns
    /// - `Result<Self>`: Returns an instance of `ChunkCode` if successful, otherwise returns an
    ///   error.
    ///
    /// # Errors
    /// - Returns an error if the language is not supported or if the `CodeSplitter` fails to build.

    #[must_use]
    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = Some(concurrency);
        self
    }


    fn detect_language(&self, path: &Path) -> &str {
        let ext = path.extension()
            .and_then(|e| e.to_str()).unwrap();

        match ext {
            "rs" => "rust",
            "py" => "python",
            "js" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            "go" => "go",
            "java" => "java",
            "c" | "cpp" | "cc" => "cpp",
            "md" => "markdown",
            _ => "text", // fallback
        }
    }
}

#[async_trait]
impl ChunkerTransformer for ChunkAdaptive {
    type Input = String;
    type Output = String;
    /// Transforms a `TextNode` by splitting its code chunk into smaller pieces.
    ///
    /// # Parameters
    /// - `node`: The `TextNode` containing the code chunk to be split.
    ///
    /// # Returns
    /// - `IndexingStream`: A stream of `TextNode` instances, each containing a smaller chunk of
    ///   code.
    ///
    /// # Errors
    /// - If the code splitting fails, an error is sent downstream.
    #[tracing::instrument(skip_all, name = "transformers.my_custom_chunker")]
    async fn transform_node(&self, node: TextNode) -> IndexingStream<String> {
        // Simply return the stream from ChunkCode
        let lang = self.detect_language(&node.path);
        match lang{
            "markdown" => {
                info!("Transforming markdown node");
                ChunkMarkdown::default().transform_node(node).await
            },
            "text" => {
                info!("Transforming markdown node");
                ChunkText::default().transform_node(node).await
            },
            _ => {
                info!("Transforming markdown node");
                ChunkCode::try_for_language_and_chunk_size(lang, 10..self.chunk_size_code).unwrap().transform_node(node).await
            }
        }
    }

    fn concurrency(&self) -> Option<usize> {
        self.concurrency
    }
}
