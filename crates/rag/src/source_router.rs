use ai_agent_common::{CollectionTier};
use futures::executor::block_on;
use ollama_rs::{OllamaClient, OllamaRequest};

/// SourceRouter with hybrid intent detection: keywords + fallback LLM classification
pub struct SourceRouter {
    ollama: OllamaClient,
}

impl SourceRouter {
    /// Create new SourceRouter with Ollama client endpoint URL
    pub fn new(ollama_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            ollama: OllamaClient::new(ollama_url),
        })
    }

    /// Fast heuristic keyword intent detection
    fn keyword_intent(&self, query: &str) -> Option<Vec<CollectionTier>> {
        let q = query.to_lowercase();

        let mut intents = Vec::new();

        if q.contains("code") || q.contains("function") || q.contains("python") || q.contains("rust") {
            intents.push(CollectionTier::Code);
        }

        if q.contains("doc") || q.contains("explain") || q.contains("reference") {
            intents.push(CollectionTier::Docs);
        }

        if q.contains("config") || q.contains("setting") {
            intents.push(CollectionTier::Config);
        }

        if q.contains("error") || q.contains("log") || q.contains("exception") {
            intents.push(CollectionTier::Logs);
        }

        if intents.is_empty() {
            None
        } else {
            Some(intents)
        }
    }

    /// Fallback async Ollama LLM call for intent classification,
    /// returns vector of CollectionTiers or empty vec for unknown
    pub async fn classify_intent_llm(&self, query: &str) -> anyhow::Result<Vec<CollectionTier>> {
        // Example prompt for zero-shot classification
        let prompt = format!(
            "Classify the query into one or more categories: 'code', 'docs', 'config', 'logs'.\nQuery: \"{}\"\nCategories:",
            query
        );

        let request = OllamaRequest::builder()
            .model("llama2_13b_chat")
            .prompt(prompt)
            .build();

        let response = self.ollama.complete(request).await?;

        let response_text = response.completions.get(0)
            .map(|c| c.text.to_lowercase())
            .unwrap_or_default();

        let mut tiers = Vec::new();

        if response_text.contains("code") {
            tiers.push(CollectionTier::Code);
        }
        if response_text.contains("docs") {
            tiers.push(CollectionTier::Docs);
        }
        if response_text.contains("config") {
            tiers.push(CollectionTier::Config);
        }
        if response_text.contains("log") {
            tiers.push(CollectionTier::Logs);
        }

        Ok(tiers)
    }

    /// Main routing function - calls fast heuristic first,
    /// falls back to LLM classification if unsure, returns queries vec.
    pub async fn route_query(
        &self,
        user_query: &str,
        ctx: &ProjectScope,
    ) -> anyhow::Result<Vec<(CollectionTier, String)>> {
        if let Some(tiers) = self.keyword_intent(user_query) {
            // Return heuristics result if confident
            let queries = tiers.into_iter()
                .map(|tier| (tier, user_query.to_string()))
                .collect();
            Ok(queries)
        } else {
            // Fallback to LLM classification
            let tiers = self.classify_intent_llm(user_query).await?;
            let queries = if tiers.is_empty() {
                vec![(CollectionTier::Code, user_query.to_string())] // default
            } else {
                tiers.into_iter().map(|tier| (tier, user_query.to_string())).collect()
            };
            Ok(queries)
        }
    }
}

// Example usage sync wrapper
impl SourceRouter {
    pub fn route_query_blocking(&self, user_query: &str, agent_ctx: &AgentContext) -> anyhow::Result<Vec<(CollectionTier, String)>> {
        block_on(self.route_query(user_query, agent_ctx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_agent_common::{AgentContext};

    #[tokio::test]
    async fn test_route_query_hybrid() {
        let router = SourceRouter::new("http://localhost:11434").unwrap();

        let agent_ctx = AgentContext {
            project_root: "my_project".to_string(),
            languages: vec!["rust".to_string()],
            file_types: vec!["source".to_string()],
        };

        // Uses heuristic
        let queries = router.route_query("how to use async in rust", &agent_ctx).await.unwrap();
        assert!(queries.iter().any(|(tier, _)| *tier == CollectionTier::Code));

        // Uses fallback LLM when heuristic returns none
        let queries2 = router.route_query("open api documentation", &agent_ctx).await.unwrap();
        assert!(queries2.iter().any(|(tier, _)| *tier == CollectionTier::Docs));
    }
}
