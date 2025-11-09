use ai_agent_common::*;
use qdrant_client::{Qdrant, points::PointStruct};
use anyhow::Result;

pub struct QdrantClient {
    inner: Qdrant,
}

impl QdrantClient {
    pub async fn new(url: &str) -> Result<Self> {
        let inner = Qdrant::new(url).await?;
        Ok(Self { inner })
    }

    pub async fn create_collection(&self, name: &str) -> Result<()> {
        todo!("Create Qdrant collection")
    }

    pub async fn insert_point(&self, collection: &str, point: PointStruct) -> Result<()> {
        todo!("Insert point into Qdrant collection")
    }
}
