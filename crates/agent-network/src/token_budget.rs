//! Token budget management

use crate::error::AgentNetworkResult;

pub struct TokenBudgetManager {
    max_tokens_per_agent: usize,
    enable_context_pruning: bool,
    enable_prompt_caching: bool,
}

impl TokenBudgetManager {
    pub fn new(
        max_tokens_per_agent: usize,
        enable_context_pruning: bool,
        enable_prompt_caching: bool,
    ) -> Self {
        Self {
            max_tokens_per_agent,
            enable_context_pruning,
            enable_prompt_caching,
        }
    }

    /// Estimate token count for text
    pub fn estimate_tokens(&self, text: &str) -> usize {
        // TODO: Week 6 - Integrate tiktoken-rs for accurate token counting
        // Rough approximation: 1 token ≈ 4 characters
        (text.len() + 3) / 4
    }

    /// Optimize context to fit within token budget
    pub fn optimize_context(&self, context: &str) -> AgentNetworkResult<String> {
        let token_count = self.estimate_tokens(context);

        if token_count <= self.max_tokens_per_agent {
            return Ok(context.to_string());
        }

        if self.enable_context_pruning {
            // TODO: Week 6 - Implement smart context pruning
            // - Keep most relevant parts
            // - Summarize or truncate less important parts
            let pruned = self.prune_context(context)?;
            Ok(pruned)
        } else {
            Ok(context.to_string())
        }
    }

    fn prune_context(&self, context: &str) -> AgentNetworkResult<String> {
        // TODO: Week 6 - Implement smart pruning logic
        // For now, simple truncation
        let max_chars = self.max_tokens_per_agent * 4;
        if context.len() > max_chars {
            Ok(context[..max_chars].to_string())
        } else {
            Ok(context.to_string())
        }
    }

    /// Check if context fits within budget
    pub fn check_budget(&self, context: &str) -> bool {
        self.estimate_tokens(context) <= self.max_tokens_per_agent
    }
}
