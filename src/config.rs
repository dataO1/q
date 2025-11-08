use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ollama: OllamaConfig,
    pub qdrant: QdrantConfig,
    pub chunking: ChunkingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_url")]
    pub url: String,
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    #[serde(default = "default_qdrant_url")]
    pub url: String,
    #[serde(default = "default_collection_name")]
    pub collection_name: String,
    #[serde(default = "default_vector_size")]
    pub vector_size: u64,
    #[serde(default = "default_num_results")]
    pub num_results: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    pub extension_to_language: HashMap<String, String>,
    #[serde(default = "default_chunk_range")]
    pub chunk_size_range: (usize, usize),
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_embedding_model() -> String {
    "nomic-embed-text".to_string()
}

fn default_qdrant_url() -> String {
    "http://localhost:6334".to_string()
}

fn default_collection_name() -> String {
    "code_search".to_string()
}

fn default_vector_size() -> u64 {
    768
}


fn default_num_results() -> u64 {
    5
}

fn default_chunk_range() -> (usize, usize) {
    (100, 2048)
}

impl Config {
    pub fn from_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read config file")?;
        toml::from_str(&content).context("Failed to parse config")
    }

    pub fn default_config() -> Self {
        Self {
            ollama: OllamaConfig {
                url: default_ollama_url(),
                embedding_model: default_embedding_model(),
            },
            qdrant: QdrantConfig {
                url: default_qdrant_url(),
                collection_name: default_collection_name(),
                vector_size: default_vector_size(),
                num_results: default_num_results(),
            },
            chunking: ChunkingConfig {
                extension_to_language: Self::default_language_map(),
                chunk_size_range: default_chunk_range(),
            },
        }
    }

    fn default_language_map() -> HashMap<String, String> {
        [
            ("rs", "rust"),
            ("py", "python"),
            ("js", "javascript"),
            ("ts", "typescript"),
            ("go", "go"),
            ("c", "c"),
            ("cpp", "cpp"),
            ("java", "java"),
            ("rb", "ruby"),
            ("md", "markdown"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }

    pub fn get_language_for_extension(&self, ext: &str) -> Option<&str> {
        self.chunking.extension_to_language.get(ext).map(|s| s.as_str())
    }
}
