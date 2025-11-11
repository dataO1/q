use ai_agent_storage::QdrantClient;
use anyhow::{Context, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use fastembed::{SparseEmbedding, SparseInitOptions, SparseModel, SparseTextEmbedding};
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
    ) -> Result<Vec<(ContextFragment, SparseEmbedding)>>;

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
    ) -> Result<Vec<(ContextFragment, SparseEmbedding)>> {
        self.client.query_collections(queries, project_scope).await
    }
}

pub struct MultiSourceRetriever {
    sources: Vec<Arc<dyn RetrieverSource + Send + Sync>>,
    embedder: Arc<SparseTextEmbedding>,
}

impl MultiSourceRetriever {
    pub async fn new(qdrant_url: &str) -> Result<Self> {
        let qdrant_client = QdrantClient::new(qdrant_url)?;
        let embedder = SparseTextEmbedding::try_new(
                SparseInitOptions::new(SparseModel::SPLADEPPV1)
                    .with_show_download_progress(true), // Optional: show download progress
            )?;

        Ok(Self {
            sources: vec![Arc::new(QdrantRetriever::new(qdrant_client))],
            embedder: Arc::new(embedder),
        })
    }
    pub fn embedder(&self) -> &Arc<SparseTextEmbedding> {
        &self.embedder
    }

    pub fn sources(&self) -> &[Arc<dyn RetrieverSource + Send + Sync>] {
        &self.sources
    }

    pub fn embed_query(
        &self,
        texts: Vec<&str>,
        _context: Option<&ProjectScope>,
    ) -> Result<Vec<SparseEmbedding>> {
        self.embedder.embed(texts, None)
    }

    /// Retrieve all sources, then rerank and deduplicate combined results before streaming out in batches
    pub fn retrieve_stream<'a>(
        &'a self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &'a ProjectScope,
    ) -> Pin<Box<dyn Stream<Item = Result<Vec<ContextFragment>>> + Send + 'a>> {
        let sources = self.sources.clone();
        let queries = queries.clone(); // clone here to move into async block

        let stream = Box::pin(try_stream! {
            // Collect all results from all sources
            let mut all_results = Vec::<(ContextFragment, SparseEmbedding)>::new();

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
            let q = vec![query_text; 1];
            let query_embedding = self.embedder.embed(q, None)?;

            // Rerank & deduplicate on combined results
            let reranked = Reranker::rerank_and_deduplicate(query_embedding.first().unwrap(), &all_results);

            // Stream reranked results in batches of 10
            for batch in reranked.as_slice().chunks(10) {
                // Clone references to get owned Vec<ContextFragment>
                let batch_owned: Vec<ContextFragment> = batch.iter()
                    .map(|fragment| fragment.clone())
                    .collect();

                yield batch_owned;
            }
        });
        stream
    }
}

#[async_trait]
impl RetrieverSource for MultiSourceRetriever {
    fn priority(&self) -> Priority {
        0
    }

    async fn retrieve(
        &self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &ProjectScope,
    ) -> Result<Vec<(ContextFragment, SparseEmbedding)>> {
        // Collect combined results from all sources
        let mut all_results = Vec::new();
        for source in self.sources.iter() {
            let partial_results = source.retrieve(queries.clone(), project_scope).await?;
            all_results.extend(partial_results);
        }
        Ok(all_results)
    }
}
