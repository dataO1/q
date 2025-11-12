use anyhow::Context;
use fastembed::{EmbeddingModel, InitOptions, SparseInitOptions, SparseModel, SparseTextEmbedding, TextEmbedding};
use swiftide_integrations::{fastembed::{EmbeddingModelType, FastEmbed, FastEmbedBuilder}, ollama::{config::OllamaConfig, Ollama}, openai::GenericOpenAI};

#[derive(Clone)]
pub struct EmbeddingClient{
    pub embedder_dense: GenericOpenAI<OllamaConfig>,
    pub embedder_sparse: FastEmbed
}
impl EmbeddingClient{
    pub fn new(dense_model: &String, sparse_model: SparseModel)-> anyhow::Result<Self>{
        let embedder_dense = Ollama::builder()
            .default_embed_model(dense_model)
            .build()
            .context("Failed to build dense embedding model client")?;
        tracing::debug!("Initializing FastEmbed sparse...");
        let sparse_options = SparseInitOptions::new(sparse_model);
        let model_sparse = SparseTextEmbedding::try_new(sparse_options)?;
        let model_type = EmbeddingModelType::Sparse(model_sparse);
        let embedder_sparse= FastEmbedBuilder::default().embedding_model(model_type).build()?;

        Ok(Self{
            embedder_sparse,
            embedder_dense
        })
    }
}
