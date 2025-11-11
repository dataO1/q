use anyhow::{Context, Result};
use async_trait::async_trait;
use qdrant_client::prelude::*;
use qdrant_client::qdrant::{
    Condition, FieldCondition, Filter, Match, MatchValue, SearchPoints,
};
use std::sync::Arc;

use ai_agent_common::{AgentContext, CollectionTier, ContextFragment, ProjectScope};

pub struct QdrantClient {
    client: Arc<qdrant_client::client::QdrantClient>,
}

impl QdrantClient {
    pub fn new(url: &str) -> Result<Self> {
        let client = qdrant_client::client::QdrantClient::from_url(url).build()?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Build Qdrant metadata filter from AgentContext
    fn build_metadata_filter(&self, agent_ctx: &AgentContext) -> Filter {
        let mut must_conditions = vec![];

        // Filter 1: Exact match on project_root
        must_conditions.push(Condition {
            condition_one_of: None,
            field: Some(FieldCondition {
                key: "project_root".to_string(),
                r#match: Some(Match {
                    match_value: Some(MatchValue::Keyword(agent_ctx.project_root.clone())),
                }),
                range: None,
                geo_bounding_box: None,
                geo_radius: None,
                values_count: None,
            }),
        });

        // Filter 2: Match any language in agent_ctx.languages (OR condition)
        if !agent_ctx.languages.is_empty() {
            let lang_matches: Vec<Condition> = agent_ctx
                .languages
                .iter()
                .map(|lang| Condition {
                    condition_one_of: None,
                    field: Some(FieldCondition {
                        key: "language".to_string(),
                        r#match: Some(Match {
                            match_value: Some(MatchValue::Keyword(lang.clone())),
                        }),
                        range: None,
                        geo_bounding_box: None,
                        geo_radius: None,
                        values_count: None,
                    }),
                })
                .collect();

            // Wrap in should condition (OR)
            if !lang_matches.is_empty() {
                must_conditions.push(Condition {
                    condition_one_of: Some(qdrant_client::qdrant::Filter {
                        should: lang_matches,
                        must: vec![],
                        must_not: vec![],
                    }),
                    field: None,
                });
            }
        }

        // Filter 3: Match any file_type in agent_ctx.file_types (OR condition)
        if !agent_ctx.file_types.is_empty() {
            let filetype_matches: Vec<Condition> = agent_ctx
                .file_types
                .iter()
                .map(|ft| Condition {
                    condition_one_of: None,
                    field: Some(FieldCondition {
                        key: "file_type".to_string(),
                        r#match: Some(Match {
                            match_value: Some(MatchValue::Keyword(ft.clone())),
                        }),
                        range: None,
                        geo_bounding_box: None,
                        geo_radius: None,
                        values_count: None,
                    }),
                })
                .collect();

            if !filetype_matches.is_empty() {
                must_conditions.push(Condition {
                    condition_one_of: Some(qdrant_client::qdrant::Filter {
                        should: filetype_matches,
                        must: vec![],
                        must_not: vec![],
                    }),
                    field: None,
                });
            }
        }

        Filter {
            must: must_conditions,
            must_not: vec![],
            should: vec![],
        }
    }

    /// Query Qdrant collections with metadata pre-filtering
    pub async fn query_collections(
        &self,
        queries: Vec<(CollectionTier, String)>,
        project_scope: &ProjectScope,
        agent_ctx: &AgentContext,
    ) -> Result<Vec<ContextFragment>> {
        let mut results = vec![];
        let filter = self.build_metadata_filter(agent_ctx);

        for (tier, query_text) in queries {
            let collection_name = tier.to_string();

            // Generate embedding vector for the query_text
            // TODO: Replace this placeholder with actual embedding generation
            let query_vector = vec![0.0f32; 384]; // Placeholder dimension

            let search_result = self
                .client
                .search_points(&SearchPoints {
                    collection_name: collection_name.clone(),
                    vector: query_vector,
                    filter: Some(filter.clone()),
                    limit: 50,
                    with_payload: Some(true.into()),
                    ..Default::default()
                })
                .await
                .context(format!("Qdrant search failed for collection {}", collection_name))?;

            // Convert Qdrant points to ContextFragments
            for scored_point in search_result.result {
                let payload = scored_point.payload;

                let fragment = ContextFragment {
                    content: payload
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    summary: payload
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    source: payload
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    score: scored_point.score,
                };

                results.push(fragment);
            }
        }

        Ok(results)
    }
}
