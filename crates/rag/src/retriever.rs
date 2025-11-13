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

use ai_agent_common::{llm::EmbeddingClient, CollectionTier, ContextFragment, ProjectScope};

pub type Priority = u8;

#[async_trait]
pub trait RetrieverSource: Send + Sync {
    fn priority(&self) -> Priority;

    async fn retrieve(
        &self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &ProjectScope,
    ) -> Result<Vec<ContextFragment>>;


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

        RetrieverSourcePrioStream{stream:Box::pin(stream),priority:self.priority()}
    }
}

pub struct QdrantRetriever<'a> {
    client: Arc<QdrantClient<'a>>,
}

impl<'a> QdrantRetriever<'a> {
    pub fn new(client: QdrantClient<'a>) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

#[async_trait]
impl<'a> RetrieverSource for QdrantRetriever<'a> {
    fn priority(&self) -> Priority {
        1
    }

    async fn retrieve(
        &self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &ProjectScope,
    ) -> Result<Vec<ContextFragment>> {
        self.client.query_collections(queries, project_scope,None).await
    }
}

pub struct MultiSourceRetriever<'a> {
    sources: Vec<Arc<dyn RetrieverSource + Send + Sync + 'a>>,
    embedder: Arc<EmbeddingClient>,
}

impl<'a> MultiSourceRetriever<'a> {
    pub async fn new(qdrant_client: &QdrantClient<'a>, embedder: &EmbeddingClient) -> Result<Self> {
        Ok(Self {
            sources: vec![Arc::new(QdrantRetriever::<'a>::new(qdrant_client.clone()))],
            embedder: Arc::new(embedder.clone()),
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
    pub fn retrieve_stream(
        &'a self,
        raw_query: &'a str,
        queries: HashMap<CollectionTier, Vec<String>>,
        project_scope: &'a ProjectScope,
    ) -> Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send + 'a>> {

        let stream = Box::pin(try_stream! {
            // Collect all results from all sources
            let mut all_streams = Vec::<RetrieverSourcePrioStream>::new();

            // let all_streams:  Vec<RetrieverSourcePrioStream> = sources.into_iter().map(|source|{
            //     source.retrieve_stream(queries.clone(), &project_scope.clone())
            // }).collect();
            for source in self.sources.iter() {
                let partial_results = source.retrieve_stream(queries.clone(), &project_scope);
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

                    reranked.sort_by_key(|f| Reverse(f.score));
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
    stream: Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send + 'a>>,
    priority: u8,
}
