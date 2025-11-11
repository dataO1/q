// reranker.rs
use anyhow::Result;
use ai_agent_common::ContextFragment;
use fastembed::SparseEmbedding;
use simsimd::{SpatialSimilarity};

const SIMILARITY_THRESHOLD: f32 = 0.85;

pub struct Reranker {}

impl Reranker {
    // Compute cosine similarity between two sparse embeddings
    pub fn cosine_similarity(a: &SparseEmbedding, b: &SparseEmbedding) -> f32 {
        let mut dot_product = 0.0;
        let mut i = 0;
        let mut j = 0;

        // Compute dot product for matching indices
        while i < a.indices.len() && j < b.indices.len() {
            if a.indices[i] == b.indices[j] {
                dot_product += a.values[i] * b.values[j];
                i += 1;
                j += 1;
            } else if a.indices[i] < b.indices[j] {
                i += 1;
            } else {
                j += 1;
            }
        }

        // Compute magnitudes using simsimd on values slices
        let mag_a = SpatialSimilarity::cosine(&a.values, &a.values).unwrap_or(0.0).sqrt() as f32;
        let mag_b = SpatialSimilarity::cosine(&b.values, &b.values).unwrap_or(0.0).sqrt() as f32;

        if mag_a == 0.0 || mag_b == 0.0 {
            return 0.0;
        }

        dot_product / (mag_a * mag_b)
    }

    pub fn rerank_and_deduplicate(
        query_embedding: &SparseEmbedding,
        candidates: &[(ContextFragment, SparseEmbedding)],
    ) -> Vec<ContextFragment> {
        let mut scored = candidates
            .iter()
            .map(|(frag, emb)| (frag, Self::cosine_similarity(query_embedding, emb)))
            .collect::<Vec<_>>();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let mut selected = Vec::new();
        let mut selected_embs: Vec<&SparseEmbedding> = Vec::new();

        for (frag, _) in scored {
            let candidate_emb = &candidates.iter().find(|(f, _)| *f == *frag).unwrap().1;
            if selected_embs.iter().all(|&emb| {
                let sim = Self::cosine_similarity(emb, &candidate_emb);
                sim < SIMILARITY_THRESHOLD
            }) {
                selected.push((*frag).clone());
                selected_embs.push(&candidate_emb);
            }
        }

        selected
    }
}
