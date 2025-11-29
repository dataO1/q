use ai_agent_storage::RedisCache;
use ai_agent_common::SystemConfig;
use anyhow::{Context, Result};
use ai_agent_common::{CollectionTier, ConversationId, ProjectScope};
use moka::future::Cache;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;
use tokenizers::pre_tokenizers::whitespace::Whitespace;
use tokenizers::processors::template::TemplateProcessing;
use tokenizers::{Tokenizer, models::wordpiece::WordPiece, normalizers::BertNormalizer};
use sha2::{Digest, Sha256};
use tracing::{debug, instrument, warn};

#[derive(Debug)]
pub struct QueryEnhancer {
    ollama_client: Ollama,
    mem_cache: Cache<String, Vec<String>>,
    redis_client: RedisCache,
    redis_cache_prefix: String,
    tokenizer: Tokenizer,
    model: String,
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

    pub async fn new(config: &SystemConfig) -> anyhow::Result<Self> {
        let redis_url = config.storage.redis_url.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Redis URL not configured"))?;

        Ok(Self {
            ollama_client: Ollama::new(&config.embedding.ollama_host, config.embedding.ollama_port),
            mem_cache: Cache::new(10000),
            redis_client: RedisCache::new(redis_url).await?,
            redis_cache_prefix: "query_enhancer_cache:".to_string(),
            tokenizer: create_bert_tokenizer(&config.rag.query_enhancer_vocab_path.to_str().unwrap())
                .map_err(|e| anyhow::Error::msg(format!("{}", e))).context("Failed to read tokenizers config file")?,
            model: config.rag.query_enhancement_model.to_string(),
        })
    }


    #[instrument(name = "query_heuristic_expansion", skip(self), fields(raw_query))]
    /// Apply simple heuristics: synonym expansions, normalization, token filtering
    fn heuristic_expand(&self, query: &str) -> anyhow::Result<Vec<String>> {
        let mut results = Vec::new();

        // 1. Original normalized query
        let norm = query.trim().to_lowercase();
        results.push(norm.clone());

        // 2. Simple synonym replacement heuristic
        if norm.contains("error") {
            results.push(norm.replace("error", "exception"));
            results.push(norm.replace("error", "bug"));
        }

        if norm.contains("async") {
            results.push(norm.replace("async", "asynchronous"));
        }

        if norm.contains("function") {
            results.push(norm.replace("function", "method"));
        }

        // 3. Token filtering with PROPER WordPiece reconstruction
        let encoding = self.tokenizer.encode(norm.as_str(), false)
            .map_err(|e| anyhow::Error::msg(format!("{}", e)))?;

        let tokens = encoding.get_tokens();

        let reconstructed_words = self.reconstruct_from_wordpiece(tokens);

        // Filter stopwords from reconstructed words
        let filtered_words: Vec<String> = reconstructed_words
            .into_iter()
            .filter(|word| !self.is_stopword(word))
            .collect();

        if !filtered_words.is_empty() {
            results.push(filtered_words.join(" "));
        }

        // Deduplicate
        results.sort();
        results.dedup();

        debug!("Computed heuristic variants: {:?}", &results);
        Ok(results)
    }

    /// Reconstruct original words from WordPiece tokens
    /// Example: ["as", "##yn", "##c", "doc"] -> ["async", "doc"]
    fn reconstruct_from_wordpiece(&self, tokens: &[String]) -> Vec<String> {
        let mut words = Vec::new();
        let mut current_word = String::new();

        for token in tokens {
            // Skip special tokens
            if token == "[CLS]" || token == "[SEP]" || token == "[PAD]" || token == "[UNK]" {
                continue;
            }

            if token.starts_with("##") {
                // This is a continuation token - merge with current word
                current_word.push_str(&token[2..]); // Remove "##" prefix
            } else {
                // Start of a new word
                if !current_word.is_empty() {
                    words.push(current_word.clone());
                }
                current_word = token.clone();
            }
        }

        // Don't forget the last word
        if !current_word.is_empty() {
            words.push(current_word);
        }

        words
    }

    fn is_stopword(&self, token: &str) -> bool {
        // Expanded stopword list for better filtering
        let stopwords = [
            "the", "is", "at", "which", "on", "a", "an",
            "and", "or", "but", "in", "with", "to", "from",
            "of", "for", "by", "as", "this", "that", "these", "those"
        ];
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
        const HEURISTIC_VERSION: u8 = 1;
        let cache_key = self.compute_cache_key(raw_query, conversation_id, tier, project_scope, HEURISTIC_VERSION);

        // Check caches...
        if let Some(cached) = self.mem_cache.get(&cache_key).await {
            debug!("Found in-memory cached result");
            return Ok(cached);
        }

        if let Ok(Some(redis_cached)) = self.redis_get(&cache_key).await {
            debug!("Found redis cached result");
            self.mem_cache.insert(cache_key.clone(), redis_cached.clone()).await;
            return Ok(redis_cached);
        }

        // Apply heuristics first
        let heuristic_variants = self.heuristic_expand(raw_query)?;
        results.extend(heuristic_variants.iter().cloned());

        // ✅ IMPROVED: Concise, focused prompt
        let prompt = match tier {
            CollectionTier::Online => {
                format!(
                    "Rewrite this search query for better web search results. \
                    Return ONLY the improved query, no explanation.\n\n\
                    Original: \"{}\"\n\
                    Context: {} project{}\n\n\
                    Improved query:",
                    raw_query,
                    project_scope.language_distribution.iter()
                        .map(|(lang, _weight)| format!("{:?}", lang))  // ✅ Fixed: iter() instead of keys()
                        .collect::<Vec<_>>()
                        .join("/"),
                    project_scope.current_file
                        .as_ref()
                        .map(|f| format!(" ({})", f.display()))
                        .unwrap_or_default()
                )
            }
            CollectionTier::Workspace => {
                format!(
                    "Rewrite this code search query for better results in a {} codebase. \
                    Return ONLY the improved query.\n\n\
                    Original: \"{}\"\n\n\
                    Improved query:",
                    project_scope.language_distribution.iter()
                        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                        .map(|(lang, _weight)| format!("{:?}", lang))
                        .unwrap_or_else(|| "unknown".to_string()),
                    raw_query
                )
            }
            CollectionTier::System | CollectionTier::Personal => {
                format!(
                    "Improve this search query for technical documentation. \
                    Return ONLY the query.\n\n\
                    Original: \"{}\"\n\n\
                    Improved:",
                    raw_query
                )
            }
        };

        // Query LLM with timeout protection
        let request = GenerationRequest::new(self.model.clone(),&prompt);
        match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.ollama_client.generate(request)
        ).await {
            Ok(Ok(enhanced)) => {
                let response = enhanced.response;
                debug!("LLM enhanced query: {}",response);
                results.push(response.clone());

                // Cache the full result set
                self.mem_cache.insert(cache_key.clone(), results.clone()).await;
                let _ = self.redis_set(&cache_key,&response, 86400).await;
            }
            Ok(Err(e)) => {
                warn!("LLM enhancement failed: {}. Using heuristics only.", e);
                // Continue with heuristics only
            }
            Err(_) => {
                warn!("LLM enhancement timed out. Using heuristics only.");
                // Continue with heuristics only
            }
        }

        Ok(results)
    }
}
