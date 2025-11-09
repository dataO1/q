use ai_agent_common::*;

pub struct OllamaEmbedder {
    client: reqwest::Client,
    model: String,
}

impl OllamaEmbedder {
    pub fn new(model: String) -> Result<Self> {
        todo!("Initialize Ollama client")
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        todo!("Generate embedding via Ollama")
    }

    pub async fn embed_batch(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
        todo!("Batch embedding")
    }
}
