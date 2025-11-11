use ai_agent_common::{CollectionTier, EmbeddingConfig, ProjectScope};
use futures::executor::block_on;
use ollama_rs::{generation::{chat::{request::ChatMessageRequest, ChatMessage, MessageRole}, completion::request::GenerationRequest, parameters::FormatType}, Ollama};
use strum::IntoEnumIterator;
use swiftide::chat_completion::ChatCompletionRequest;
use tracing::info; // You must bring the trait into scope

/// SourceRouter with hybrid intent detection: keywords + fallback LLM classification
pub struct SourceRouter {
    ollama: Ollama,
}

impl SourceRouter {
    /// Create new SourceRouter with Ollama client endpoint URL
    pub fn new(config: &EmbeddingConfig) -> anyhow::Result<Self> {
        Ok(Self {
            ollama: Ollama::new(&config.ollama_host, config.ollama_port),
        })
    }

    /// Fast heuristic keyword intent detection

    /// Fallback async Ollama LLM call for intent classification,
    /// returns vector of CollectionTiers or empty vec for unknown
    pub async fn classify_intent_llm(&self, query: &str) -> anyhow::Result<Vec<CollectionTier>> {
        // Example prompt for zero-shot classification
        let categories: Vec<String> = CollectionTier::iter().into_iter().map(|tier| format!("{:?}",tier)).collect();

        let template = format!(r#"{{
          "type": "array",
          "categories": "{:?}"
        }}"#,categories);

        let system_prompt = format!(
        "You are a precise classification system. Your ONLY task is to classify user queries into one or more of these categories: {:?}. You must respond ONLY with valid JSON in the exact format specified. Do not include any explanations, comments, or additional text outside the JSON structure.",
            categories,
        );
        let user_prompt = format!(
        "
        Classify the following query into one or more categories:

        Categories: {:?}

        Query: {:?}

        Output only valid JSON with the classification results.
        ",
            categories,
            query
        );
        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: system_prompt,
                tool_calls: vec![],
                images: None,
                thinking: None
            },
            ChatMessage {
                role: MessageRole::User,
                content: user_prompt,
                tool_calls: vec![],
                images: None,
                thinking: None
            },
        ];
        let request = ChatMessageRequest::new( "phi3-mini".to_string(), messages,).template(template);
        let response = self.ollama.send_chat_messages(request).await?.message.content;
        info!("Classification: {}", response);
        let items: Vec<CollectionTier> = serde_json::from_str(&response)?;
        Ok(items)
    }

    /// Main routing function - calls fast heuristic first,
    /// falls back to LLM classification if unsure, returns queries vec.
    pub async fn route_query(
        &self,
        user_query: &str,
        ctx: &ProjectScope,
    ) -> anyhow::Result<Vec<(CollectionTier, String)>> {
        // Fallback to LLM classification
        let tiers = self.classify_intent_llm(user_query).await?;
        let queries = if tiers.is_empty() {
            vec![(CollectionTier::Workspace, user_query.to_string())] // default
        } else {
            tiers.into_iter().map(|tier| (tier, user_query.to_string())).collect()
        };
        Ok(queries)
    }
}
