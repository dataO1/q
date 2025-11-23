//! Context formatting and aggregation from RAG and History
//!
//! Converts ContextFragment streams and HistoryContext into formatted strings
//! suitable for agent context injection.

use ai_agent_common::{ContextFragment, ConversationId};
use futures::{Stream, StreamExt, FutureExt};
use std::pin::Pin;
use tracing::{debug, info, instrument, Instrument};

use crate::error::AgentNetworkResult;

/// Formatted RAG context ready for agent consumption
#[derive(Debug, Clone)]
pub struct FormattedRagContext {
    /// Combined formatted context from all fragments
    pub content: String,

    /// Number of fragments retrieved
    pub fragment_count: usize,

    /// Approximate token count (rough estimate)
    pub estimated_tokens: usize,

    /// Priority-ordered sources (workspace, personal, system, online)
    pub source_tiers: Vec<String>,
}

impl FormattedRagContext {
    /// Create empty RAG context
    pub fn empty() -> Self {
        Self {
            content: String::new(),
            fragment_count: 0,
            estimated_tokens: 0,
            source_tiers: vec![],
        }
    }

    /// Check if context is empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// Formatted History context ready for agent consumption
#[derive(Debug, Clone)]
pub struct FormattedHistoryContext {
    /// Combined formatted history context
    pub content: String,

    /// Short-term memory (recent exchanges)
    pub short_term: Vec<String>,

    /// Relevant past context from semantic search
    pub relevant_past: Vec<String>,

    /// Summary of conversation
    pub summary: Option<String>,

    /// Topics mentioned in history
    pub topics: Vec<String>,

    /// Approximate token count
    pub estimated_tokens: usize,
}

impl FormattedHistoryContext {
    /// Create empty history context
    pub fn empty() -> Self {
        Self {
            content: String::new(),
            short_term: vec![],
            relevant_past: vec![],
            summary: None,
            topics: vec![],
            estimated_tokens: 0,
        }
    }

    /// Check if context is empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// Context builder for collecting and formatting streams
pub struct ContextBuilder;

impl ContextBuilder {
    /// Collect and format RAG context stream into string
    ///
    /// # Arguments
    /// * `stream` - Stream of ContextFragments from RAG
    /// * `max_tokens` - Maximum token budget for context
    ///
    /// # Returns
    /// FormattedRagContext with combined content and metadata
    #[instrument(skip(stream), fields(max_tokens))]
    pub async fn build_rag_context(
        stream: Pin<Box<dyn Stream<Item = anyhow::Result<ContextFragment>> + Send>>,
        max_tokens: usize,
    ) -> AgentNetworkResult<FormattedRagContext> {
        debug!("Building RAG context from stream (max_tokens: {})", max_tokens);

        let mut formatted = String::new();
        let mut fragment_count = 0;
        let mut token_count = 0;
        let source_tiers = std::collections::HashSet::new();

        let mut stream = std::pin::pin!(stream);
        
        // Capture current span context for stream processing
        let current_span = tracing::Span::current();

        while let Some(Ok(fragment)) = stream.next().await {
            let fragment_span = tracing::info_span!(
                parent: &current_span,
                "rag_fragment", 
                source_tier = ?fragment.metadata.location,
                content_length = fragment.content.len(),
                fragment_count = fragment_count + 1
            );
            let _enter = fragment_span.enter();

            // Check token budget
            let fragment_tokens = Self::estimate_tokens(&fragment.content);
            if token_count + fragment_tokens > max_tokens {
                debug!("Token budget exceeded, stopping fragment collection (current: {}, needed: {}, max: {})", 
                    token_count, fragment_tokens, max_tokens);
                break;
            }
            
            debug!("Processing RAG fragment (tokens: {}, content preview: {}...)", 
                fragment_tokens, 
                fragment.content.chars().take(100).collect::<String>()
            );

            // Format fragment with location info
            let location = fragment.metadata.location;
            // let tier_name = format!("{:?}", location.tier);
            // source_tiers.insert(tier_name);

            formatted.push_str(&format!(
                "## {}\n",
                serde_json::to_string(&location)?
            ));

            // Add content
            formatted.push_str(&fragment.content);
            formatted.push_str("\n\n");

            token_count += fragment_tokens;
            fragment_count += 1;
        }

        if fragment_count == 0 {
            debug!("No RAG fragments retrieved");
            return Ok(FormattedRagContext::empty());
        }

        let source_tiers_vec: Vec<String> = source_tiers.into_iter().collect();

        info!(
            "RAG context built: {} fragments, ~{} tokens, source tiers: {:?}",
            fragment_count, token_count, source_tiers_vec
        );

        Ok(FormattedRagContext {
            content: formatted,
            fragment_count,
            estimated_tokens: token_count,
            source_tiers: source_tiers_vec,
        })
    }

    /// Format history context from HistoryManager result
    #[instrument(skip(history_result))]
    pub async fn build_history_context(
        history_result: ai_agent_history::manager::HistoryContext,
    ) -> AgentNetworkResult<FormattedHistoryContext> {
        debug!("Building history context");

        let mut formatted = String::new();
        let mut token_count = 0;
        let mut short_term_vec = Vec::new();
        let mut relevant_past_vec = Vec::new();
        let mut summary_vec = Vec::new();

        // Add short-term memory
        if !history_result.short_term.is_empty() {
            formatted.push_str("## Recent Conversation\n");
            for msg in &history_result.short_term {
                let message_string = serde_json::to_string(msg)?;
                short_term_vec.push(message_string.clone());
                formatted.push_str(&format!("- {}\n",message_string));
                token_count += Self::estimate_tokens(&message_string);
            }
            formatted.push_str("\n");
        }

        // Add relevant past context
        if !history_result.relevant_past.is_empty() {
            formatted.push_str("## Relevant Past Context\n");
            for msg in &history_result.relevant_past {
                let message_string = serde_json::to_string(msg)?;
                relevant_past_vec.push(message_string.clone());
                formatted.push_str(&format!("- {}\n",message_string));
                token_count += Self::estimate_tokens(&message_string);
            }
            formatted.push_str("\n");
        }

        // Add summary if available
        if let Some(summary) = &history_result.summary {
            formatted.push_str("## Conversation Summary\n");
            formatted.push_str(summary);
            formatted.push_str("\n\n");
            token_count += Self::estimate_tokens(summary);
            summary_vec.push(summary);
        }

        // Add topics
        if !history_result.topics.is_empty() {
            formatted.push_str("## Topics Discussed\n");
            formatted.push_str(&history_result.topics.join(", "));
            formatted.push_str("\n\n");
        }

        info!(
            "History context built: {} short-term messages, {} relevant past messages, {} topics, ~{} tokens",
            short_term_vec.len(), relevant_past_vec.len(), history_result.topics.len(), token_count
        );

        Ok(FormattedHistoryContext {
            content: formatted,
            short_term: short_term_vec,
            relevant_past: relevant_past_vec,            summary: history_result.summary.clone(),
            topics: history_result.topics.clone(),
            estimated_tokens: token_count,
        })
    }

    /// Estimate token count (rough: 1 token ≈ 4 characters)
    pub fn estimate_tokens(text: &str) -> usize {
        (text.len() / 4).max(1)
    }

    /// Combine RAG and history contexts with token budget
    pub fn combine_contexts(
        rag: FormattedRagContext,
        history: FormattedHistoryContext,
        max_total_tokens: usize,
    ) -> String {
        let mut combined = String::new();
        let mut current_tokens = 0;

        // Prioritize RAG context
        if current_tokens + rag.estimated_tokens <= max_total_tokens {
            if !rag.content.is_empty() {
                combined.push_str("# Retrieved Context\n\n");
                combined.push_str(&rag.content);
                current_tokens += rag.estimated_tokens;
            }
        }

        // Add history if space available
        if current_tokens + history.estimated_tokens <= max_total_tokens {
            if !history.content.is_empty() {
                combined.push_str("# Conversation History\n\n");
                combined.push_str(&history.content);
                current_tokens += history.estimated_tokens;
            }
        } else {
            debug!(
                "History context truncated due to token budget (available: {}, needed: {})",
                max_total_tokens - current_tokens,
                history.estimated_tokens
            );
        }

        combined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let text = "This is a test string";
        let tokens = ContextBuilder::estimate_tokens(text);
        assert!(tokens > 0);
    }

    #[test]
    fn test_formatted_rag_context_empty() {
        let ctx = FormattedRagContext::empty();
        assert!(ctx.is_empty());
        assert_eq!(ctx.fragment_count, 0);
    }

    #[test]
    fn test_formatted_history_context_empty() {
        let ctx = FormattedHistoryContext::empty();
        assert!(ctx.is_empty());
    }
}
