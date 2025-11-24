//! Smart Multi-Source RAG System with Priority-Based Batched Streaming

pub mod context_manager;
pub mod context_providers;
pub mod query_enhancer;
pub mod source_router;
pub mod retriever;
pub mod reranker;

use ai_agent_common::llm::EmbeddingClient;
use ai_agent_storage::QdrantClient;
use anyhow::{Context, Result};
use futures::{Stream};
use tracing::{debug, instrument};
use std::pin::Pin;
use std::sync::Arc;
use std::collections::HashMap;

use ai_agent_common::{CollectionTier, ContextFragment, ConversationId, ProjectScope, SystemConfig};
use crate::retriever::MultiSourceRetriever;
use crate::query_enhancer::QueryEnhancer;

#[derive(Debug)]
/// Main RAG pipeline struct
pub struct SmartMultiSourceRag {
    query_enhancer: QueryEnhancer,
    source_router: source_router::SourceRouter,
    retriever: Arc<MultiSourceRetriever>,
}

impl SmartMultiSourceRag {
    /// Initialize RAG cores
    pub async fn new(config: &SystemConfig, embedder: Arc<EmbeddingClient>) -> anyhow::Result<Arc<Self>> {
        let qdrant_client = Arc::new(QdrantClient::new(&config.storage.qdrant_url,embedder.clone())?);
        Ok(Arc::new(Self {
            query_enhancer: QueryEnhancer::new(&config.storage.redis_url.as_ref().unwrap()).await?,
            source_router: source_router::SourceRouter::new(&config)?,
            retriever: Arc::new(MultiSourceRetriever::new(qdrant_client, embedder).await?),
        }))
    }

    #[instrument(name = "rag_query_enhancement", skip(self), fields(source_queries))]
    async fn enhance_queries(
        &self,
        source_queries: &HashMap<CollectionTier, String>,
        project_scope: &ProjectScope,
        conversation_id: &ConversationId
    ) -> Result<HashMap<CollectionTier, Vec<String>>> {
        let futures = source_queries.iter().map(|(tier, source_query)| async move {
            let result_queries = self.query_enhancer
                .enhance(source_query, project_scope, conversation_id, tier.clone())
                .await.unwrap_or(vec![source_query.to_string()]);
            (tier.clone(), result_queries)
        }).collect::<Vec<_>>();

        let results: Vec<(CollectionTier, Vec<String>)> = futures::future::join_all(futures).await;

        // Convert Vec of tuples into HashMap
        let enhanced_queries: HashMap<CollectionTier, Vec<String>> = results.into_iter().collect();

        Ok(enhanced_queries)
    }

    #[instrument(name = "rag_streaming_retrieval", skip(self), fields(raw_query, project_scope, conversation_id))]
    /// Runs the multi-stage priority batched streaming retrieval pipeline
    pub async fn retrieve_stream(
        self:Arc<Self>,
        raw_query: String,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ContextFragment>> + Send + 'static>>> {

        let rag = Arc::clone(&self);

        // Step 1: Route query to sources
        let source_queries = rag.source_router.route_query(&raw_query, &project_scope)
            .await.context("Source routing failed")?;

        debug!("Generated source specific queries: {:?}", source_queries);

        // Enhance per tier (parallel, context-aware)
        let enhanced_queries = rag.enhance_queries(&source_queries, &project_scope,&conversation_id).await.context("Enhancing queries failed!")?;
        debug!("Generated enhanced queries for source specific queries: {:?}", source_queries);

        // Step 3: Prepare priority-ordered streams from MultiSourceRetriever
        let prioritized_streams = rag.retriever.clone().retrieve_stream(raw_query,enhanced_queries, project_scope);
        Ok(prioritized_streams)
    }
}
