//! RAG and History context integration into agent workflow
//!
//! Manages retrieval, formatting, and injection of contextual information
//! from SmartMultiSourceRag and HistoryManager into agent execution contexts.

pub mod context_builder;

pub use context_builder::{ContextBuilder, FormattedRagContext, FormattedHistoryContext};

use ai_agent_common::{ConversationId, ProjectScope};
use ai_agent_rag::SmartMultiSourceRag;
use ai_agent_history::manager::HistoryManager;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, Instrument};

use crate::error::AgentNetworkResult;

/// Context provider for agent execution
///
/// Orchestrates RAG and History retrieval based on task-specific queries
#[derive(Debug)]
pub struct ContextProvider {
    rag: Arc<SmartMultiSourceRag>,
    history_manager: Arc<RwLock<HistoryManager>>,
    token_budget: usize,
}

impl ContextProvider {
    /// Create new context provider
    pub fn new(
        rag: Arc<SmartMultiSourceRag>,
        history_manager: Arc<RwLock<HistoryManager>>,
        token_budget: usize,
    ) -> Self {
        Self {
            rag,
            history_manager,
            token_budget,
        }
    }

    /// Retrieve context for a specific task using agent-specific refined query
    ///
    /// This implements Agentic RAG pattern:
    /// - Uses task.description (already refined by orchestrator)
    /// - Retrieves RAG context via SmartMultiSourceRag
    /// - Retrieves history context via HistoryManager
    /// - Combines and formats for agent consumption
    ///
    /// # Arguments
    /// * `task_query` - Agent-specific refined query from task.description
    ///
    /// # Returns
    /// Combined formatted context string ready for AgentContext injection
    #[instrument(skip(self), fields(query_len = %task_query.len(), token_budget = self.token_budget))]
    pub async fn retrieve_context(&self,
        task_query: String,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
        ) -> AgentNetworkResult<String> {
        info!("Retrieving context for task query: {}", task_query);

        // Retrieve RAG context via stream with proper span instrumentation
        let rag_future = self.retrieve_rag_context(task_query.clone(), project_scope, conversation_id.clone())
            .instrument(tracing::info_span!("rag_retrieval", query = %task_query));
        let rag_context = rag_future.await?;

        // Retrieve history context with proper span instrumentation  
        let history_future = self.retrieve_history_context(task_query, conversation_id)
            .instrument(tracing::info_span!("history_retrieval"));
        let history_context = history_future.await?;

        // Combine contexts with token budget
        let combined = ContextBuilder::combine_contexts(
            rag_context,
            history_context,
            self.token_budget,
        );

        if combined.is_empty() {
            debug!("No context retrieved");
        } else {
            info!(
                "Context retrieval complete: {} bytes",
                combined.len()
            );
        }

        Ok(combined)
    }

    /// Retrieve RAG context from SmartMultiSourceRag
    #[instrument(skip(self), fields(
        rag.query = %query,
        token_budget = self.token_budget / 2
    ))]
    async fn retrieve_rag_context(&self,
        query: String,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
        ) -> AgentNetworkResult<FormattedRagContext> {
        debug!("Querying RAG with refined query: {}", query);

        // Call SmartMultiSourceRag::retrieve_stream with task-specific query
        let stream = self.rag.clone().retrieve_stream(
            query,
            project_scope,
            conversation_id,
        ).await?;

        // Collect and format stream
        let rag_context = ContextBuilder::build_rag_context(
            stream,
            self.token_budget / 2, // Allocate half budget to RAG
        ).await?;

        Ok(rag_context)
    }

    /// Retrieve history context from HistoryManager
    #[instrument(skip(self))]
    async fn retrieve_history_context(&self, query: String, conversation_id: ConversationId) -> AgentNetworkResult<FormattedHistoryContext> {
        debug!("Querying history manager");

        let history = self.history_manager.read().await;

        // Call get_relevant_context with same refined query
        let history_result = history.get_relevant_context(
            &conversation_id,
            query,
        ).await?;

        // Format history context
        let history_context = ContextBuilder::build_history_context(history_result).await?;

        Ok(history_context)
    }

    /// Store new exchange in history after task completion
    ///
    /// Called after agent execution to update history with new interaction
    pub async fn store_exchange(
        &self,
        user_query: String,
        agent_response: String,
        conversation_id: ConversationId
    ) -> AgentNetworkResult<()> {
        debug!("Storing exchange in history");

        let mut history = self.history_manager.write().await;

        history.add_exchange(
            &conversation_id,
            user_query,
            agent_response,
        ).await?;

        info!("Exchange stored in history");
        Ok(())
    }
}

