use ai_agent_storage::QdrantClient;
use anyhow::{Context, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;

use ai_agent_common::{CollectionTier, ContextFragment, ProjectScope};
use crate::{reranker::Reranker};

pub type Priority = u8;

#[async_trait]
pub trait RetrieverSource: Send + Sync {
    fn priority(&self) -> Priority;

    async fn retrieve(
        &self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &ProjectScope,
    ) -> Result<Vec<(ContextFragment, SparseVector<f32>)>>;

    fn retrieve_stream<'a>(
        &'a self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &'a ProjectScope,
    ) -> Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send + 'a>> {
        let queries_clone = queries.clone();
        let project_scope_clone = project_scope.clone();

        let stream = try_stream! {
            let results = self.retrieve(queries_clone, &project_scope_clone).await?;
            for (frag, _emb) in results {
                yield frag;
            }
        };

        Box::pin(stream)
    }
}

pub struct QdrantRetriever {
    client: Arc<QdrantClient>,
}

impl QdrantRetriever {
    pub fn new(client: QdrantClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

#[async_trait]
impl RetrieverSource for QdrantRetriever {
    fn priority(&self) -> Priority {
        1
    }

    async fn retrieve(
        &self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &ProjectScope,
        agent_ctx: &AgentContext,
    ) -> Result<Vec<(ContextFragment, SparseVector<f32>)>> {
        self.client.query_collections(queries, project_scope, agent_ctx).await
    }
}

pub struct MultiSourceRetriever {
    sources: Vec<Arc<dyn RetrieverSource>>,
}

impl MultiSourceRetriever {
pub async fn new(qdrant_url: &str) -> Result<Self> {
        let qdrant_client = QdrantClient::new(qdrant_url)?;
        let embedder = Embedder::load(embedder_model_path).await?;

        Ok(Self {
            sources: vec![Arc::new(QdrantRetriever::new(qdrant_client))],
            embedder: Arc::new(embedder),
        })
    }

    /// Retrieve all sources, then rerank and deduplicate combined results before streaming out in batches
    pub fn retrieve_stream<'a>(
        &'a self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &'a ProjectScope,
    ) -> Pin<Box<dyn Stream<Item = Result<Vec<ContextFragment>>> + Send + 'a>> {
        let sources = self.sources.clone();

        Box::pin(try_stream! {
            // Collect all results from all sources
            let mut all_results = Vec::<(ContextFragment, SparseVector<f32>)>::new();

            for source in sources.into_iter() {
                let partial_results = source.retrieve(queries.clone(), project_scope).await?;
                all_results.extend(partial_results);
            }

            if all_results.is_empty() {
                yield Vec::new();
                return;
            }

            // Generate query embedding for reranking - TODO replace with actual embedding
            let query_text = queries.get(0).map(|(_, q)| q.as_str()).unwrap_or("");
            let query_embedding = SparseVector::from_dense(&vec![0f32; 384]);

            // Rerank & deduplicate on combined results
            let reranked = Reranker::rerank_and_deduplicate(&query_embedding, &all_results);

            // Stream reranked results in batches (e.g., batches of 10)
            for batch in &reranked.into_iter().chunks(10) {
                let batch_vec: Vec<_> = batch.collect();
                yield batch_vec;
            }
        })
    }
}
