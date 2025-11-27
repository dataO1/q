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
use tracing::{debug, info, instrument, warn};

use ai_agent_common::{llm::EmbeddingClient, CollectionTier, ContextFragment, Location, ProjectScope, SystemConfig};
use crate::web_crawler::WebCrawlerRetriever;

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

// Helper function to create a retriever stream from any RetrieverSource with simplified span instrumentation
pub fn create_retriever_stream(
    source: Arc<dyn RetrieverSource>,
    queries: HashMap<CollectionTier, Vec<String>>,
    project_scope: ProjectScope,
) -> RetrieverSourcePrioStream {
    let total_queries: usize = queries.values().map(|v| v.len()).sum();

    // Get source name for debugging (using Debug trait)
    let source_name = format!("{:?}", source);

    debug!("Starting retrieval from {} (priority: {}, total queries: {})",
        source_name, source.priority(), total_queries);

    let start_time = std::time::Instant::now();

    // Map each (tier, queries) to an async future that retrieves vectors & converts to Vec<Result<ContextFragment>>
    let fetch_futures = queries.into_iter().map(|(tier, q_list)| {
        let tier = tier.clone();
        let q_list = q_list.clone();
        let project_scope = project_scope.clone();
        let s_self = Arc::clone(&source); // Arc clone, 'static safe
        async move {
            debug!("Querying {:?} tier with {} queries", tier, q_list.len());

            // Retrieve results: Vec<ContextFragment>
            let results = s_self
                .retrieve(
                    q_list.into_iter().map(|q| (tier.clone(), q)).collect(),
                    project_scope.clone(),
                )
                .await?;

            debug!("Retrieved {} fragments from {:?} tier", results.len(), tier);

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
        .flatten()
        .map(move |result| {
            // Count successful fragments for metrics
            if let Ok(_fragment) = &result {
                // We could track per-fragment metrics here if needed
            }
            result
        });

    // Record completion metrics
    let duration = start_time.elapsed();

    debug!("RetrieverSource stream created for {} (latency: {}ms)", source_name, duration.as_millis());

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
        // Filter out Online/System tiers
        let local_queries: Vec<_> = queries
            .into_iter()
            .filter(|(tier, _)|
                matches!(*tier,
                    CollectionTier::Personal |
                    CollectionTier::Workspace |
                    CollectionTier::System))
            .collect();

        if local_queries.is_empty() {
            return Ok(vec![]);
        }

        match self.client.query_collections(local_queries, &project_scope, None).await {
            Ok(results) => Ok(results),
            Err(e) => {
                // Log the error but don't crash the stream
                warn!("QdrantRetriever failed: {}. Returning empty results.", e);
                Ok(vec![]) // Graceful degradation
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MultiSourceRetriever {
    sources: Vec<Arc<dyn RetrieverSource>>,
    embedder: Arc<EmbeddingClient>,
}

impl MultiSourceRetriever {
    pub async fn new(
        qdrant_client: Arc<QdrantClient>,
        embedder: Arc<EmbeddingClient>,
        redis_client: Arc<ai_agent_storage::RedisCache>,
        system_config: SystemConfig,
    ) -> Result<Self> {
        let mut sources: Vec<Arc<dyn RetrieverSource>> = vec![
            Arc::new(QdrantRetriever::new(qdrant_client.clone()))
        ];

        // Add web crawler retriever if enabled
        if system_config.rag.web_crawler.enabled {
            match WebCrawlerRetriever::new(qdrant_client.clone(), redis_client, embedder.clone(), system_config).await {
                Ok(web_crawler) => {
                    sources.push(Arc::new(web_crawler));
                    info!("Successfully initialized web crawler retriever");
                }
                Err(e) => {
                    warn!("Failed to initialize web crawler retriever: {}", e);
                    // Continue without web crawler - graceful degradation
                }
            }
        } else {
            info!("Web crawler disabled in configuration");
        }

        Ok(Self {
            sources,
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

    /// Retrieve all sources in parallel, rerank combined results, then stream them
    /// This ensures consumers get best results first regardless of source priority
    pub fn retrieve_stream(
        self: Arc<Self>,
        raw_query: String,
        queries: HashMap<CollectionTier, Vec<String>>,
        project_scope: ProjectScope,
    ) -> Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send>> {
        let sources = self.sources.clone();
        let queries = queries.clone();
        let project_scope = project_scope.clone();

        let stream = Box::pin(try_stream! {
            let fetch_start = std::time::Instant::now();

            // Launch parallel retrieval from all sources
            let mut fetch_futures = Vec::new();

            for source in sources.iter() {
                let source = Arc::clone(source);
                let queries = queries.clone();
                let project_scope = project_scope.clone();
                let source_name = format!("{:?}", source);

                // Create future for this source
                let fut = async move {
                    let source_start = std::time::Instant::now();

                    // Convert HashMap<Tier, Vec<String>> to Vec<(Tier, String)>
                    let query_vec: Vec<(CollectionTier, String)> = queries
                        .into_iter()
                        .flat_map(|(tier, q_list)| {
                            q_list.into_iter().map(move |q| (tier.clone(), q))
                        })
                        .collect();

                    debug!(
                        "Fetching from source {:?} with {} queries",
                        source_name,
                        query_vec.len()
                    );

                    // Retrieve from this source
                    let results = match source.retrieve(query_vec, project_scope).await {
                        Ok(fragments) => {
                            let duration = source_start.elapsed();
                            info!(
                                "Source {:?} returned {} fragments in {:?}",
                                source_name,
                                fragments.len(),
                                duration
                            );
                            fragments
                        }
                        Err(e) => {
                            // Log error but don't fail entire stream
                            warn!(
                                "Source {:?} failed: {}. Continuing with other sources.",
                                source_name,
                                e
                            );
                            vec![]
                        }
                    };

                    // Return results with priority metadata
                    Ok::<(u8, Vec<ContextFragment>), anyhow::Error>((
                        source.priority(),
                        results
                    ))
                };

                fetch_futures.push(fut);
            }

            // Wait for all sources with timeout
            let timeout = tokio::time::Duration::from_secs(60);
            let all_results = match tokio::time::timeout(
                timeout,
                futures::future::join_all(fetch_futures)
            ).await {
                Ok(results) => {
                    let fetch_duration = fetch_start.elapsed();
                    debug!(
                        "All sources completed in {:?}",
                        fetch_duration
                    );
                    results
                }
                Err(_) => {
                    warn!(
                        "Timeout waiting for sources after {:?}. Using partial results.",
                        timeout
                    );
                    // This shouldn't happen with join_all, but handle it anyway
                    vec![]
                }
            };

            // Flatten results and apply priority-based relevance boost
            let mut all_fragments: Vec<ContextFragment> = Vec::new();
            let mut source_stats: HashMap<u8, usize> = HashMap::new();

            for result in all_results {
                match result {
                    Ok((priority, mut fragments)) => {
                        let count = fragments.len();
                        *source_stats.entry(priority).or_insert(0) += count;

                        // Apply priority boost to relevance scores
                        // Priority 1 (local) → 2.0x boost
                        // Priority 2 → 1.5x boost
                        // Priority 3 (web) → 1.0x (no boost)
                        let boost_factor = match priority {
                            1 => 2.0,
                            2 => 1.5,
                            _ => 1.0,
                        };

                        for fragment in fragments.iter_mut() {
                            fragment.relevance_score =
                                (fragment.relevance_score as f32 * boost_factor) as usize;
                        }

                        all_fragments.extend(fragments);
                    }
                    Err(e) => {
                        warn!("Source fetch failed: {}", e);
                    }
                }
            }

            info!(
                "Retrieved {} total fragments from sources: {:?}",
                all_fragments.len(),
                source_stats
            );

            if all_fragments.is_empty() {
                warn!("No fragments retrieved from any source");
                return;
            }

            // Rerank: Sort by boosted relevance score (descending)
            all_fragments.sort_by_key(|f| std::cmp::Reverse(f.relevance_score));

            // Optional: Apply diversity to avoid too many results from same source
            // (Uncomment if needed)
            all_fragments = self.apply_diversity_filter(all_fragments, 5);

            debug!(
                "Reranked {} fragments, streaming in relevance order",
                all_fragments.len()
            );

            // Stream reranked results
            for (idx, fragment) in all_fragments.into_iter().enumerate() {
                debug!(
                    "Yielding fragment {} with score {} from {:?}",
                    idx + 1,
                    fragment.relevance_score,
                    fragment.metadata.location
                );
                yield fragment;
            }

            info!("Stream completed successfully");
        });

        stream
    }
    /// Apply diversity filter to prevent single source domination
    fn apply_diversity_filter(
        &self,
        fragments: Vec<ContextFragment>,
        max_per_source: usize
    ) -> Vec<ContextFragment> {
        use std::collections::HashMap;

        let mut source_counts: HashMap<String, usize> = HashMap::new();
        let mut filtered = Vec::new();

        for fragment in fragments {
            // Use location type as source identifier
            let source_key = match &fragment.metadata.location {
                Location::File { .. } => "local_file",
                Location::WebContent { .. } => "web_content",
                _ => "other",
            };

            let count = source_counts.entry(source_key.to_string()).or_insert(0);

            if *count < max_per_source {
                *count += 1;
                filtered.push(fragment);
            }
        }

        filtered
    }
}

pub struct RetrieverSourcePrioStream {
    stream: Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send>>,
    priority: u8,
}
