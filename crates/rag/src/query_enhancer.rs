use ai_agent_storage::RedisCache;
use anyhow::{Context, Result};
use ai_agent_common::{CollectionTier, ConversationId, ProjectScope};
use moka::future::Cache;
use tokenizers::pre_tokenizers::whitespace::Whitespace;
use tokenizers::processors::template::TemplateProcessing;
use tokenizers::{Tokenizer, models::wordpiece::WordPiece, normalizers::BertNormalizer};
use sha2::{Digest, Sha256};
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct QueryEnhancer {
    ollama_client: OllamaClient,
    mem_cache: Cache<String, Vec<String>>,
    redis_client: RedisCache,
    redis_cache_prefix: String,
    tokenizer: Tokenizer,
}

fn create_bert_tokenizer(vocab_path: &str) -> tokenizers::Result<Tokenizer> {
    let wordpiece = WordPiece::from_file(vocab_path)
        .unk_token("[UNK]".into())
        .build()?;

    let mut tokenizer = Tokenizer::new(wordpiece);

    // Set BERT-style normalizer
    tokenizer.with_normalizer(Some(BertNormalizer::default()));

    // Set whitespace pre-tokenizer (common choice)
    tokenizer.with_pre_tokenizer(Some(Whitespace::default()));

    // Set post-processor to add special tokens like [CLS], [SEP]
    let template_processing = TemplateProcessing::builder()
        .try_single("[CLS] $A [SEP]")?
        .try_pair("[CLS] $A [SEP] $B [SEP]")?
        .special_tokens(vec![
            (String::from("[CLS]"), 101u32),
            (String::from("[SEP]"), 102u32),
        ])
        .build()?;
    tokenizer.with_post_processor(Some(template_processing));

    Ok(tokenizer)
}

impl QueryEnhancer {

    pub async fn new(redis_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            ollama_client: OllamaClient::new(),
            mem_cache: Cache::new(10000),
            redis_client: RedisCache::new(redis_url).await?,
            redis_cache_prefix: "query_enhancer_cache:".to_string(),
            tokenizer: create_bert_tokenizer(&"vocab.txt") // adjust vocab path
                .map_err(|e| anyhow::Error::msg(format!("{}", e))).context("Failed to read tokenizers config file")?,
        })
    }


    #[instrument(name = "query_heuristic_expansion", skip(self), fields(raw_query))]
    /// Apply simple heuristics: synonym expansions, normalization, token filtering
    fn heuristic_expand(&self, query: &str) -> anyhow::Result<Vec<String>> {
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
        let encoding = self.tokenizer.encode(norm, false)
            .map_err(|e| anyhow::Error::msg(format!("{}", e)))?;
        let tokens = encoding.get_tokens();
        let filtered_tokens: Vec<&str> = tokens.iter()
            .filter(|t| !self.is_stopword(t))
            .map(|t| t.as_str())
            .collect();
        results.push(filtered_tokens.join(" "));

        debug!("Computed heuristic variants: {:?}",&results);
        Ok(results)
    }

    fn is_stopword(&self, token: &str) -> bool {
        // Simplified stopword check
        let stopwords = ["the", "is", "at", "which", "on", "a"];
        stopwords.contains(&token)
    }

    async fn redis_get(&self, key: &str) -> Result<Option<Vec<String>>> {
        let full_key = format!("{}{}", self.redis_cache_prefix, key);
        self.redis_client.get(&full_key).await
    }

    async fn redis_set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<()> {
        let full_key = format!("{}{}", self.redis_cache_prefix, key);
        self.redis_client.set_ex(&full_key, value, ttl_secs).await?;
        Ok(())
    }

    fn compute_cache_key(
        &self,
        raw_query: &str,
        conversation_id: &ConversationId,
        tier: CollectionTier,
        project_scope: &ProjectScope,
        heuristic_version: u8,
    ) -> String {
        let key_str = format!(
            "{}|{}|{:?}|{:?}|v{}",
            raw_query, conversation_id,tier, project_scope.language_distribution, heuristic_version
        );
        hex::encode(Sha256::digest(key_str.as_bytes()))
    }

    #[instrument(name = "query_enhancement_full", skip(self), fields(raw_query))]
    /// Fully enhanced multi-source per-source query generator with layered cache including heuristics and LLM enhancement
    pub async fn enhance(
        &self,
        raw_query: &str,
        project_scope: &ProjectScope,
        conversation_id: &ConversationId,
        tier: CollectionTier,
    ) -> Result<Vec<String>> {
        let mut results = Vec::<String>::new();

        const HEURISTIC_VERSION: u8 = 1; // increment on heuristic changes

        let cache_key = self.compute_cache_key(raw_query, conversation_id,tier, project_scope, HEURISTIC_VERSION);
        debug!("Computed cache key: {}", cache_key);

        // Check in-memory cache first
        if let Some(cached) = self.mem_cache.get(&cache_key).await {
            debug!("Found in-memory cached result: {:?}", &cached);
            return Ok(cached);
        }

        // Check Redis cache
        if let Ok(Some(redis_cached)) = self.redis_get(&cache_key).await {
            debug!("Found redis cached result: {:?}", &redis_cached);
            self.mem_cache.insert(cache_key.clone(), redis_cached.clone()).await;
            return Ok(redis_cached);
        }

        // Apply heuristics to produce query variants
        let mut heuristic_variants = self.heuristic_expand(raw_query)?;

        // TODO: generate structured data and multiple queries here
        // Compose LLM prompt with heuristic variants + source description + context
        let prompt = format!(
            "Generate an enhanced search query optimized for this source:\n{:?}\n\nRaw query and heuristic variants:\n{:?}\n\nProject context:\n{:?}\nConversation ID:\n{}",
            tier, heuristic_variants, project_scope, conversation_id
        );
// Query Ollama LLM for final enhanced query
        let enhanced = self.ollama_client.query(&prompt).await?;
        debug!("LLM enhanced query: {}", &enhanced);

        results.append(&mut heuristic_variants);
        results.push(enhanced.clone());
        // Cache enhanced query
        self.mem_cache.insert(cache_key.clone(),results.clone()).await;
        let _ = self.redis_set(&cache_key, &enhanced, 86400).await;


        Ok(results)
    }
}

#[derive(Debug)]
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
