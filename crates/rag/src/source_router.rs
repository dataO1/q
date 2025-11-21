use std::collections::HashMap;

use ai_agent_common::{CollectionTier, ProjectScope, SystemConfig};
use ollama_rs::{generation::{chat::{request::ChatMessageRequest, ChatMessage, MessageRole}, parameters::{FormatType, JsonStructure}}, Ollama};
use strum::IntoEnumIterator;
use tracing::{debug, info, instrument}; // You must bring the trait into scope

#[derive(Debug)]
/// SourceRouter with hybrid intent detection: keywords + fallback LLM classification
pub struct SourceRouter {
    ollama: Ollama,
    classification_model: String,
}

impl SourceRouter {
    /// Create new SourceRouter with Ollama client endpoint URL
    pub fn new(config: &SystemConfig) -> anyhow::Result<Self> {
        Ok(Self {
            ollama: Ollama::new(&config.embedding.ollama_host, config.embedding.ollama_port),
            classification_model: config.rag.classification_model.clone()
        })
    }

    /// Fast heuristic keyword intent detection

    #[instrument(skip(self), fields(query))]
    /// Fallback async Ollama LLM call for intent classification,
    /// returns vector of CollectionTiers or empty vec for unknown
    pub async fn classify_intent_llm(&self, query: &str) -> anyhow::Result<Vec<CollectionTier>> {
        // Generate JSON schema structure from Rust type
        let json_structure = JsonStructure::new::<Vec<CollectionTier>>();

        let categories: Vec<String> = CollectionTier::iter()
            .filter(|x| x.to_string() == CollectionTier::Workspace.to_string()) // TODO: remove
                                                                                // this, this is
                                                                                // just for testing
            .map(|tier| tier.to_string())
            .collect();

        let categories_display = serde_json::to_string(&categories)?;

        let system_prompt = format!(
            "You are a precise classification system. Your ONLY task is to classify user queries into one or more of these categories: {}. You must respond ONLY with valid JSON conforming to the given schema. No explanations or extra text.",
            categories_display
        );

        let user_prompt = format!(
            "Classify this query into one or more categories:\n\nCategories: {}\n\nQuery: {}\n\nOutput JSON array only.",
            categories_display,
            query
        );

        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: system_prompt,
                tool_calls: vec![],
                images: None,
                thinking: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: user_prompt,
                tool_calls: vec![],
                images: None,
                thinking: None,
            },
        ];

        debug!("Querying intent classification model [{}]: {:?}", &self.classification_model, &messages);
        let request = ChatMessageRequest::new(self.classification_model.clone(), messages)
            .format(FormatType::StructuredJson(Box::new(json_structure)));

        let response = self.ollama.send_chat_messages(request).await?.message.content;

        debug!("Classification Result: {}", response);

        // Deserialize response into your output type
        let classification_output: Vec<CollectionTier> = serde_json::from_str(&response)?;

        Ok(classification_output)
    }

    #[instrument(skip(self), fields(user_query))]
    /// Main routing function - calls fast heuristic first,
    /// falls back to LLM classification if unsure, returns queries vec.
    pub async fn route_query(
        &self,
        user_query: &str,
        ctx: &ProjectScope,
    ) -> anyhow::Result<HashMap<CollectionTier, String>> {
        // Fallback to LLM classification
        let tiers = self.classify_intent_llm(user_query).await?;
        let res = tiers.into_iter().map(|tier| (tier,  user_query.to_string())).collect();
        Ok(res)
    }
}
