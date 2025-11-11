//! Smart Multi-Source RAG System with Priority-Based Batched Streaming

pub mod context_manager;
pub mod context_providers;
pub mod query_enhancer;
pub mod source_router;
pub mod retriever;
pub mod reranker;

use anyhow::{Context, Result};
use async_stream::try_stream;
use futures::{Stream, StreamExt};
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;

use ai_agent_common::{CollectionTier, ContextFragment, ConversationId, ProjectScope, SystemConfig};
use crate::reranker::Reranker;
use crate::retriever::{MultiSourceRetriever, Priority};
use crate::query_enhancer::QueryEnhancer;

/// Main RAG pipeline struct
pub struct SmartMultiSourceRag {
    context_manager: context_manager::ContextManager,
    query_enhancer: QueryEnhancer,
    source_router: source_router::SourceRouter,
    retriever: MultiSourceRetriever,
}

impl SmartMultiSourceRag {
    /// Initialize RAG cores
    pub async fn new(config: &SystemConfig) -> anyhow::Result<Self> {
        Ok(Self {
            context_manager: context_manager::ContextManager::new().await?,
            query_enhancer: QueryEnhancer::new(&config.storage.redis_url.as_ref().unwrap()).await?,
            source_router: source_router::SourceRouter::new(&config.embedding)?,
            retriever: MultiSourceRetriever::new(&config.storage.qdrant_url).await?,
        })
    }

    /// Runs the multi-stage priority batched streaming retrieval pipeline
    pub fn retrieve_stream<'a>(
        &'a self,
        raw_query: &'a str,
        project_scope: &'a ProjectScope,
        conversation_id: &'a ConversationId,
    ) -> Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send + 'a>> {

        let rag = Arc::new(self);

        let stream = try_stream! {
            // Step 1: Enhance query
            let enhanced_query = rag.query_enhancer.enhance(raw_query, project_scope, conversation_id)
                .await.context("Query enhancement failed")?;

            // Step 2: Route query to sources
            let source_queries = rag.source_router.route_query(&enhanced_query, project_scope)
                .await.context("Source routing failed")?;

            // Step 3: Prepare priority-ordered streams from MultiSourceRetriever
            let prioritized_streams = rag.retriever.retrieve_stream(source_queries, project_scope);

            // Group streams by priority in a BTreeMap to ensure ascending order of priority
            let mut streams_by_priority: BTreeMap<Priority, Vec<_>> = BTreeMap::new();
            for ps in prioritized_streams {
                streams_by_priority.entry(ps.priority).or_default().push(ps.stream);
            }

            // Step 4: For each priority level, wait until all streams complete, collect all fragments,
            //         yield entire batch downstream grouped by priority
            for (_priority, mut streams) in streams_by_priority {
                let mut batch = Vec::new();

                // Drain all streams for this priority concurrently
                for mut stream in streams.drain(..) {
                    while let Some(fragment) = stream.next().await {
                        let fragment = fragment?;
                        batch.push(fragment);
                    }
                }

                if !batch.is_empty() {
                    // Step 5: Rerank batch of fragments before yield
                    let reranked = Reranker::rerank_and_deduplicate(query_embedding,batch);

                    for fragment in reranked {
                        yield fragment;
                    }
                }
            }
        };

        Box::pin(stream)
    }
}
