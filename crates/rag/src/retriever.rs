use ai_agent_storage::QdrantClient;
use anyhow::{Context, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use fastembed::{SparseEmbedding, SparseInitOptions, SparseModel, SparseTextEmbedding};
use futures::{future::join_all, stream::{iter, select_all, FuturesUnordered}, Stream};
use tokio::time::sleep;
use tokio_stream::StreamMap;
use std::{collections::BTreeMap, time::Duration};
use std::pin::Pin;
use std::sync::Arc;
use std::collections::HashMap;
use futures::stream::StreamExt;

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
        queries: HashMap<CollectionTier, Vec<String>>,
        project_scope: &'a ProjectScope,
    ) -> RetrieverSourcePrioStream<'a>
    {
        // Map each (tier, queries) to an async future that retrieves vectors & converts to Vec<Result<ContextFragment>>
        let fetch_futures = queries.into_iter().map(|(tier, q_list)| {
            let project_scope = project_scope.clone();
            let s_self = self;
            async move {
                // Retrieve results: Vec<(ContextFragment, SparseEmbedding)>
                let results = s_self
                    .retrieve(
                        q_list.into_iter().map(|q| (tier.clone(), q)).collect(),
                        &project_scope,
                    )
                    .await?;

                // Map to Vec<Result<ContextFragment>>
                let fragments = results.into_iter().map(|(frag, emb)| Ok((frag,emb))).collect::<Vec<_>>();

                Ok::<_, anyhow::Error>(fragments)
            }
        });

        // Run all retrieval futures concurrently
        let fut_unordered = FuturesUnordered::from_iter(fetch_futures);

        // Turn each Vec<Result<ContextFragment>> into a stream; on error, stream yields single Err
        let stream = fut_unordered
            .filter_map(|res| async {
                match res {
                    Ok(vec) => Some(iter(vec).boxed()),
                    Err(e) => Some(futures::stream::once(async { Err(e) }).boxed()),
                }
            })
            .flatten();

        RetrieverSourcePrioStream{stream:Box::pin(stream),priority:self.priority()}
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


    async fn process_stream(&self, mut stream: impl Stream<Item = Result<(ContextFragment,SparseEmbedding)>> + Unpin) {
        while let Some(item) = stream.next().await {
            // println!("Processing item: {}", item);
            // Simulate some asynchronous work
            // sleep(Duration::from_millis((item * 100) as u64)).await;
        }
        println!("Stream finished.");
    }

    /// Retrieve all sources, then rerank and deduplicate combined results before streaming out in batches
    pub fn retrieve_stream<'a>(
        &'a self,
        raw_query: &'a str,
        queries: HashMap<CollectionTier, Vec<String>>,
        project_scope: &'a ProjectScope,
    ) -> Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send + 'a>> {

        let stream = Box::pin(try_stream! {
            let sources = self.sources.clone();
            let queries = queries.clone(); // clone here to move into async block
            let raw_query = raw_query.clone(); // clone here to move into async block
            let project_scope = project_scope.clone(); // clone here to move into async block
            // Collect all results from all sources
            // let mut all_streams = Vec::<RetrieverSourcePrioStream>::new();
            let all_streams:  Vec<RetrieverSourcePrioStream> = sources.into_iter().map(move |source|{
                source.retrieve_stream(queries, &project_scope)
            }).collect();
            // for source in sources.into_iter() {
            //     let partial_results = source.retrieve_stream(queries.clone(), project_scope);
            //     all_streams.push(partial_results.clone());
            // }
            let query_embedding = self.embedder.embed(vec![raw_query], None)?.get(0).unwrap();

            // Group streams by priority in a BTreeMap to ensure ascending order of priority
            let mut streams_by_priority: BTreeMap<Priority, Vec<_>> = BTreeMap::new();
            for ps in all_streams {
                streams_by_priority.entry(ps.priority).or_default().push(ps.stream);
            }

            //  yield entire batch downstream grouped by priority
            for (_priority, mut streams) in streams_by_priority {
                let mut batch: Vec<(ContextFragment,SparseEmbedding)> = Vec::new();

                // Drain all streams for this priority concurrently
                for mut stream in streams.drain(..) {
                    while let Some(fragment) = stream.next().await {
                        let fragment = fragment?;
                        batch.push(fragment);
                    }
                }
                if !batch.is_empty() {
                    // Step 5: Rerank batch of fragments before yield
                    let reranked = Reranker::rerank_and_deduplicate(&query_embedding,&batch);

                    for fragment in reranked {
                        yield fragment;
                    }
                }
            }
        });
        stream
    }
}

struct RetrieverSourcePrioStream<'a> {
    stream: Pin<Box<dyn Stream<Item = Result<(ContextFragment,SparseEmbedding)>> + Send + 'a>>,
    priority: u8,
}
