use anyhow::Result;
use qdrant_client::Qdrant;
use qdrant_client::config::QdrantConfig;

pub struct QdrantClient {
    inner: Qdrant,
}

impl QdrantClient {
    pub fn new(url: &str) -> Result<Self> {
        let config = QdrantConfig::from_url(url);
        let inner = Qdrant::new(config)?;  // No .await - it's sync
        Ok(Self { inner })
    }

    pub async fn create_collection(&self, name: &str) -> Result<()> {
        todo!("Create Qdrant collection")
    }

    pub async fn insert_point(&self, collection: &str, point: serde_json::Value) -> Result<()> {
        todo!("Insert point into Qdrant collection")
    }
}
