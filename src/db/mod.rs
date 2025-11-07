use anyhow::{Context, Result};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, VectorParamsBuilder,
};
use qdrant_client::Qdrant;

pub async fn ensure_collection(
    client: &Qdrant,
    collection_name: &str,
    vector_size: u64,
) -> Result<()> {
    // Check if collection exists
    let collections = client.list_collections().await
        .context("Failed to list collections")?;

    let exists = collections
        .collections
        .iter()
        .any(|c| c.name == collection_name);

    if !exists {
        tracing::info!("Creating collection: {}", collection_name);
        client
            .create_collection(
                CreateCollectionBuilder::new(collection_name)
                    .vectors_config(VectorParamsBuilder::new(vector_size, Distance::Cosine)),
            )
            .await
            .context("Failed to create collection")?;
    }

    Ok(())
}
