use anyhow::{Context, Result};
use ai_agent_common::{ProjectScope, ConversationId};
use async_trait::async_trait;
use moka::future::Cache;
use redis::AsyncCommands;
use rust_tokenizers::{tokenizer::Tokenizer, BertTokenizer};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct QueryEnhancer {
    ollama_client: OllamaClient,
    mem_cache: Cache<String, String>,
    redis_client: redis::Client,
    redis_cache_prefix: String,
    tokenizer: BertTokenizer,
}

impl QueryEnhancer {
    pub fn new(redis_url: &str) -> Result<Self> {
        Ok(Self {
            ollama_client: OllamaClient::new(),
            mem_cache: Cache::new(10000),
            redis_client: redis::Client::open(redis_url)?,
            redis_cache_prefix: "query_enhancer_cache:".to_string(),
            tokenizer: BertTokenizer::from_file("path/to/vocab.txt", false).unwrap(), // adjust vocab path
        })
    }

    /// Apply simple heuristics: synonym expansions, normalization, token filtering
    fn heuristic_expand(&self, query: &str) -> Vec<String> {
        let mut results = Vec::new();

        // Normalize whitespace
        let norm = query.trim().to_lowercase();
        results.push(norm.clone());

        // Simple synonym replacement heuristic
        if norm.contains("error") {
            results.push(norm.replace("error", "exception"));
            results.push(norm.replace("error", "bug"));
        }

        // Token filter heuristic (remove stopwords)
        let tokens = self.tokenizer.tokenize(&norm);
        let filtered_tokens: Vec<&str> = tokens.iter()
            .filter(|t| !self.is_stopword(t))
            .map(|t| t.as_str())
            .collect();
        results.push(filtered_tokens.join(" "));

        results
    }

    fn is_stopword(&self, token: &str) -> bool {
        // Simplified stopword check
        let stopwords = ["the", "is", "at", "which", "on", "a"];
        stopwords.contains(&token)
    }

    async fn redis_get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let full_key = format!("{}{}", self.redis_cache_prefix, key);
        Ok(conn.get(full_key).await?)
    }

    async fn redis_set(&self, key: &str, value: &str, ttl_secs: usize) -> Result<()> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let full_key = format!("{}{}", self.redis_cache_prefix, key);
        let _: () = conn.set_ex(full_key, value, ttl_secs).await?;
        Ok(())
    }

    fn compute_cache_key(
        &self,
        raw_query: &str,
        conversation_id: &ConversationId,
        source_name: &str,
        source_desc: &str,
        project_scope: &ProjectScope,
        heuristic_version: u8,
    ) -> String {
        let key_str = format!(
            "{}|{}|{}|{}|{:?}|v{}",
            raw_query, conversation_id, source_name, source_desc, project_scope.language_distribution, heuristic_version
        );
        hex::encode(Sha256::digest(key_str.as_bytes()))
    }

    /// Fully enhanced multi-source per-source query generator with layered cache including heuristics and LLM enhancement
    pub async fn enhance_for_sources(
        &self,
        raw_query: &str,
        project_scope: &ProjectScope,
        conversation_id: &ConversationId,
        source_descriptions: &[(&str, &str)],
    ) -> Result<HashMap<String, String>> {
        let mut results = HashMap::new();

        const HEURISTIC_VERSION: u8 = 1; // increment on heuristic changes

        for (source_name, description) in source_descriptions.iter() {
            let cache_key = self.compute_cache_key(raw_query, conversation_id, source_name, description, project_scope, HEURISTIC_VERSION);

            // Check in-memory cache first
            if let Some(cached) = self.mem_cache.get(&cache_key) {
                results.insert(source_name.to_string(), cached.clone());
                continue;
            }

            // Check Redis cache
            if let Ok(Some(redis_cached)) = self.redis_get(&cache_key).await {
                self.mem_cache.insert(cache_key.clone(), redis_cached.clone()).await;
                results.insert(source_name.to_string(), redis_cached);
                continue;
            }

            // Apply heuristics to produce query variants
            let heuristic_variants = self.heuristic_expand(raw_query);

            // Compose LLM prompt with heuristic variants + source description + context
            let prompt = format!(
                "Generate an enhanced search query optimized for this source:\n{}\n\nRaw query and heuristic variants:\n{:?}\n\nProject context:\n{:?}\nConversation ID:\n{}",
                description, heuristic_variants, project_scope, conversation_id
            );

            // Query Ollama LLM for final enhanced query
            let enhanced = self.ollama_client.query(&prompt).await?;

            // Cache enhanced query
            self.mem_cache.insert(cache_key.clone(), enhanced.clone()).await;
            let _ = self.redis_set(&cache_key, &enhanced, 86400).await;

            results.insert(source_name.to_string(), enhanced);
        }

        Ok(results)
    }
}

pub struct OllamaClient {}

impl OllamaClient {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn query(&self, prompt: &str) -> Result<String> {
        // Implement actual HTTP or SDK call to Ollama API here.
        Ok(format!("LLM enhanced query for prompt: {}", prompt))
    }
}
