use ai_agent_storage::QdrantClient;
use anyhow::Result;
use async_stream::try_stream;
use async_trait::async_trait;
use futures::{stream::{iter, FuturesUnordered}, Stream};
use std::{cmp::Reverse, collections::BTreeMap};
use std::pin::Pin;
use std::sync::Arc;
use std::collections::HashMap;
use futures::stream::StreamExt;
use swiftide::{SparseEmbedding};
use tracing::{debug, instrument};

use ai_agent_common::{llm::EmbeddingClient, CollectionTier, ContextFragment, ProjectScope};

pub type Priority = u8;

#[async_trait]
pub trait RetrieverSource: std::fmt::Debug + Send + Sync + 'static {
    fn priority(&self) -> Priority;

    async fn retrieve(
        &self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: ProjectScope,
    ) -> Result<Vec<ContextFragment>>;
}

// Helper function to create a retriever stream from any RetrieverSource
pub fn create_retriever_stream(
    source: Arc<dyn RetrieverSource>,
    queries: HashMap<CollectionTier, Vec<String>>,
    project_scope: ProjectScope,
) -> RetrieverSourcePrioStream {
    // Map each (tier, queries) to an async future that retrieves vectors & converts to Vec<Result<ContextFragment>>
    let fetch_futures = queries.into_iter().map(|(tier, q_list)| {
        let tier = tier.clone();
        let q_list = q_list.clone();
        let project_scope = project_scope.clone();
        let s_self = Arc::clone(&source); // Arc clone, 'static safe
        async move {
            // Retrieve results: Vec<ContextFragment>
            let results = s_self
                .retrieve(
                    q_list.into_iter().map(|q| (tier, q)).collect(),
                    project_scope.clone(),
                )
                .await?;

            // Map to Vec<Result<ContextFragment>>
            let fragments = results.into_iter().map(|frag| Ok(frag)).collect::<Vec<_>>();

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

    RetrieverSourcePrioStream {
        stream: Box::pin(stream),
        priority: source.priority(),
    }
}

#[derive(Debug)]
pub struct QdrantRetriever {
    client: Arc<QdrantClient>,
}

impl QdrantRetriever {
    pub fn new(client: Arc<QdrantClient>) -> Self {
        Self {
            client: client,
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
        project_scope: ProjectScope,
    ) -> Result<Vec<ContextFragment>> {
        self.client.query_collections(queries, &project_scope,None).await
    }
}

#[derive(Debug)]
pub struct MultiSourceRetriever {
    sources: Vec<Arc<dyn RetrieverSource>>,
    embedder: Arc<EmbeddingClient>,
}

impl MultiSourceRetriever {
    pub async fn new(qdrant_client: Arc<QdrantClient>, embedder: Arc<EmbeddingClient>) -> Result<Self> {
        Ok(Self {
            sources: vec![Arc::new(QdrantRetriever::new(qdrant_client.clone()))],
            embedder: embedder.clone(),
        })
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
    #[instrument(skip(self), fields(raw_query, query_tiers = queries.len(), sources = self.sources.len()))]
    pub fn retrieve_stream(
        self:Arc<Self>,
        raw_query: String,
        queries: HashMap<CollectionTier, Vec<String>>,
        project_scope: ProjectScope,
    ) -> Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send>> {
        let sources = self.sources.clone();
        let queries = queries.clone();
        let project_scope = project_scope.clone();

        let stream = Box::pin(try_stream! {
            // Collect all results from all sources
            let mut all_streams = Vec::<RetrieverSourcePrioStream>::new();

            // let all_streams:  Vec<RetrieverSourcePrioStream> = sources.into_iter().map(|source|{
            //     source.retrieve_stream(queries.clone(), &project_scope.clone())
            // }).collect();
            for source in sources.iter() {
                let partial_results = create_retriever_stream(source.clone(), queries.clone(), project_scope.clone());
                all_streams.push(partial_results);
            }
            // let query_embeddings = self.embedder.embedder_sparse.embed(vec![raw_query.to_string()]).await?;
            // let query_embedding = query_embeddings.get(0).unwrap();
            //
            // Group streams by priority in a BTreeMap to ensure ascending order of priority
            let mut streams_by_priority: BTreeMap<Priority, Vec<_>> = BTreeMap::new();
            for ps in all_streams {
                streams_by_priority.entry(ps.priority).or_default().push(ps.stream);
            }

            //  yield entire batch downstream grouped by priority
            for (_priority, mut streams) in streams_by_priority {
                let mut batch: Vec<ContextFragment> = Vec::new();

                // Drain all streams for this priority concurrently
                for mut stream in streams.drain(..) {
                    while let Some(fragment) = stream.next().await {
                        let fragment = fragment?;
                        batch.push(fragment);
                    }
                }
                if !batch.is_empty() {
                    // Step 5: Rerank batch of fragments before yield
                    // let reranked = Reranker::rerank_and_deduplicate(&query_embedding,&batch);
                    let mut reranked = batch;

                    reranked.sort_by_key(|f| Reverse(f.relevance_score));
                    for fragment in reranked {
                        yield fragment;
                    }
                }
            }
        });
        stream
    }
}

pub struct RetrieverSourcePrioStream {
    stream: Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send>>,
    priority: u8,
}
