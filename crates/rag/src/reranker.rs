// reranker.rs
use anyhow::Result;
use ai_agent_common::ContextFragment;
use fastembed::SparseVector;

const SIMILARITY_THRESHOLD: f32 = 0.85;

pub struct Reranker {}

impl Reranker {
    pub fn cosine_similarity(a: &SparseVector<f32>, b: &SparseVector<f32>) -> f32 {
        a.cosine_similarity(b)
    }

    pub fn rerank_and_deduplicate(
        query_embedding: &SparseVector<f32>,
        candidates: &[(ContextFragment, SparseVector<f32>)],
    ) -> Vec<ContextFragment> {
        let mut scored = candidates
            .iter()
            .map(|(frag, emb)| (frag, Self::cosine_similarity(query_embedding, emb)))
            .collect::<Vec<_>>();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let mut selected = Vec::new();
        let mut selected_embs = Vec::new();

        for (frag, _) in scored {
            if selected_embs.iter().all(|emb| {
                let sim = Self::cosine_similarity(emb, candidates.iter().find(|(f, _)| f == *frag).unwrap().1);
                sim < SIMILARITY_THRESHOLD
            }) {
                selected.push((*frag).clone());
                selected_embs.push(candidates.iter().find(|(f, _)| f == *frag).unwrap().1.clone());
            }
        }

        selected
    }
}
