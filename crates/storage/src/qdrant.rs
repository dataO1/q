use anyhow::{Context, Result};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, VectorParamsBuilder,
    UpsertPointsBuilder, SearchPointsBuilder, PointStruct,
    Value, Condition, Filter, DeletePointsBuilder,
};
use std::collections::HashMap;

pub struct QdrantClient {
    inner: Qdrant,
}

impl QdrantClient {
    /// Create new Qdrant client
    pub fn new(url: &str) -> Result<Self> {
        let inner = Qdrant::from_url(url)
            .build()
            .context("Failed to create Qdrant client")?;

        tracing::info!("Connected to Qdrant at {}", url);
        Ok(Self { inner })
    }

/// Create a collection with the given name and vector size
    pub async fn create_collection(&self, name: &str, vector_size: u64) -> Result<()> {
        use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParamsBuilder};

        self.inner.create_collection(
            CreateCollectionBuilder::new(name)
                .vectors_config(VectorParamsBuilder::new(vector_size, Distance::Cosine))
        ).await
            .context(format!("Failed to create collection: {}", name))?;

        tracing::info!("Created collection: {}", name);
        Ok(())
    }


    pub async fn delete_collection(&self, name: &str) -> Result<()> {
        self.inner.delete_collection(name).await
            .context(format!("Failed to create collection: {}", name))?;

        tracing::info!("Created collection: {}", name);
        Ok(())
    }

    /// Check if collection exists
    pub async fn collection_exists(&self, name: &str) -> Result<bool> {
        match self.inner.collection_info(name).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Ensure collection exists, create if missing
    pub async fn ensure_collection(&self, name: &str, vector_size: u64) -> Result<()> {
        if !self.collection_exists(name).await? {
            self.create_collection(name, vector_size).await?;
        }
        Ok(())
    }

    /// Insert a point (document chunk) into collection
    pub async fn insert_point(
        &self,
        collection: &str,
        id: u64,
        vector: Vec<f32>,
        payload: HashMap<String, Value>,
    ) -> Result<()> {
        let point = PointStruct::new(id, vector, payload);

        let upsert = UpsertPointsBuilder::new(collection, vec![point])
            .build();

        self.inner
            .upsert_points(upsert)
            .await
            .context("Failed to insert point")?;

        Ok(())
    }

    /// Batch insert multiple points
    pub async fn insert_points(
        &self,
        collection: &str,
        points: Vec<(u64, Vec<f32>, HashMap<String, Value>)>,
    ) -> Result<()> {
        let qdrant_points: Vec<PointStruct> = points
            .into_iter()
            .map(|(id, vector, payload)| PointStruct::new(id, vector, payload))
            .collect();

        let upsert = UpsertPointsBuilder::new(collection, qdrant_points)
            .build();

        self.inner
            .upsert_points(upsert)
            .await
            .context("Failed to batch insert points")?;

        tracing::debug!("Inserted batch into collection: {}", collection);
        Ok(())
    }

    /// Search for similar vectors in collection
    pub async fn search(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        limit: u64,
        filter: Option<Filter>,
    ) -> Result<Vec<SearchResult>> {
        let mut search_builder = SearchPointsBuilder::new(collection, query_vector, limit);

        if let Some(f) = filter {
            search_builder = search_builder.filter(f);
        }

        let search_result = self.inner
            .search_points(search_builder.build())
            .await
            .context("Failed to search points")?;

        let results = search_result
            .result
            .into_iter()
            .map(|scored_point| {
                // Extract ID - can be either Num or Uuid
                let id = if let Some(point_id) = scored_point.id {
                    if let Some(point_id_options) = point_id.point_id_options {
                        use qdrant_client::qdrant::point_id::PointIdOptions;
                        match point_id_options {
                            PointIdOptions::Num(n) => n,
                            PointIdOptions::Uuid(_) => 0, // Convert UUID to 0 for now
                        }
                    } else {
                        0
                    }
                } else {
                    0
                };

                SearchResult {
                    id,
                    score: scored_point.score,
                    payload: scored_point.payload,
                }
            })
            .collect();

        Ok(results)
    }

    /// Search with metadata filters (e.g., project_root, language, file_type)
    pub async fn search_with_metadata(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        limit: u64,
        project_root: Option<&str>,
        language: Option<&str>,
        file_types: Option<Vec<&str>>,
    ) -> Result<Vec<SearchResult>> {
        let mut must_conditions: Vec<Condition> = Vec::new();

        // Filter by project root using matches()
        if let Some(root) = project_root {
            must_conditions.push(Condition::matches("project_root", root.to_string()));
        }

        // Filter by language
        if let Some(lang) = language {
            must_conditions.push(Condition::matches("language", lang.to_string()));
        }

        // Filter by file types (OR condition)
        if let Some(types) = file_types {
            if !types.is_empty() {
                let type_strings: Vec<String> = types.iter().map(|t| t.to_string()).collect();
                // Use matches with vec for any matching
                must_conditions.push(Condition::matches("file_type", type_strings));
            }
        }

        let filter = if !must_conditions.is_empty() {
            Some(Filter {
                must: must_conditions,
                ..Default::default()
            })
        } else {
            None
        };

        self.search(collection, query_vector, limit, filter).await
    }

    /// Delete points by filter
    pub async fn delete_by_filter(&self, collection: &str, filter: Filter) -> Result<()> {
        let delete_request = DeletePointsBuilder::new(collection)
            .points(filter)  // Use .points() with Filter, not .points_selector()
            .wait(true)
            .build();

        self.inner
            .delete_points(delete_request)
            .await
            .context("Failed to delete points")?;

        Ok(())
    }
}

/// Search result from Qdrant
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: u64,
    pub score: f32,
    pub payload: HashMap<String, Value>,
}

impl SearchResult {
    /// Extract string field from payload
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.payload.get(key)?.as_str().map(|s| s.to_string())
    }

    /// Extract integer field from payload
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.payload.get(key)?.as_integer()
    }
}
