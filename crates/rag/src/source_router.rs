use ai_agent_common::*;

pub struct SourceRouter {
    heuristic_router: HeuristicRouter,
    llm_router: Option<LlmRouter>,
}

impl SourceRouter {
    pub fn new() -> Result<Self> {
        todo!("Initialize routers")
    }

    pub async fn select_sources(
        &self,
        query: &str,
        task_type: TaskType,
    ) -> Result<Vec<CollectionTier>> {
        todo!("Select sources via heuristics or LLM")
    }
}

struct HeuristicRouter;
struct LlmRouter;

impl HeuristicRouter {
    fn route(&self, query: &str, task_type: TaskType) -> Option<Vec<CollectionTier>> {
        todo!("Fast keyword-based routing")
    }
}
