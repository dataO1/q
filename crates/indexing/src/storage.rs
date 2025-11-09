use ai_agent_common::*;
use qdrant_client::Qdrant;

pub struct QdrantStorage {
    client: Qdrant,
}

impl QdrantStorage {
    pub async fn new(url: &str) -> Result<Self> {
        todo!("Initialize Qdrant client")
    }

    pub async fn create_collections(&self) -> Result<()> {
        todo!("Create all collection tiers")
    }

    pub async fn insert_chunk(
        &self,
        tier: CollectionTier,
        chunk: &crate::chunker::Chunk,
        embedding: Vec<f32>,
        metadata: DocumentMetadata,
    ) -> Result<()> {
        todo!("Insert chunk into Qdrant")
    }
}
