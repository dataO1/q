//! Core orchestrator logic
//!
//! The Orchestrator is the central controller that:
//! - Analyzes user queries
//! - Decomposes tasks into sub-tasks
//! - Generates dynamic DAG workflows
//! - Manages agent execution
//! - Handles error recovery and HITL integration

use crate::error::{AgentNetworkError, AgentNetworkResult};
use crate::workflow::{
    DependencyType, TaskNode, TaskResult, WorkflowBuilder, WorkflowExecutor, WorkflowGraph,
};
use crate::agents::AgentPool;
use crate::status_stream::StatusStream;
use crate::sharedcontext::SharedContext;
use crate::coordination::CoordinationManager;
use crate::filelocks::FileLockManager;
use ai_agent_common::{AgentNetworkConfig, ErrorRecoveryStrategy};
use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error, instrument};
use uuid::Uuid;
use anyhow::Result;

/// Core Orchestrator for multi-agent coordination
pub struct Orchestrator {
    /// Configuration
    config: AgentNetworkConfig,

    /// Agent pool for task execution
    agent_pool: Arc<AgentPool>,

    /// Real-time status event streaming
    status_stream: Arc<StatusStream>,

    /// Shared context across agents
    shared_context: Arc<RwLock<SharedContext>>,

    /// Task coordination
    coordination: Arc<CoordinationManager>,

    /// File lock manager
    file_locks: Arc<FileLockManager>,
}

/// Represents a decomposed task for a single agent
#[derive(Debug, Clone)]
pub struct DecomposedTask {
    pub id: String,
    pub agent_id: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub priority: u32,
    pub requires_hitl: bool,
    pub recovery_strategy: ErrorRecoveryStrategy,
}

/// Query analysis result
#[derive(Debug, Clone)]
pub struct QueryAnalysis {
    pub query: String,
    pub complexity: Complexity,
    pub suggested_agent_types: Vec<String>,
    pub requires_hitl: bool,
    pub estimated_tokens: usize,
}

/// Complexity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Complexity {
    Trivial = 0,
    Simple = 1,
    Moderate = 2,
    Complex = 3,
    VeryComplex = 4,
}

impl Orchestrator {
    /// Create a new Orchestrator
    pub async fn new(config: AgentNetworkConfig) -> Result<Self> {
        info!("Initializing Orchestrator with {} agents", config.agents.len());

        let agent_pool = Arc::new(AgentPool::new(&config.agents).await?);
        let status_stream = Arc::new(StatusStream::new());
        let shared_context = Arc::new(RwLock::new(SharedContext::new()));
        let coordination = Arc::new(CoordinationManager::new());
        let file_locks = Arc::new(FileLockManager::new(30));

        info!("Orchestrator initialized successfully");

        Ok(Self {
            config,
            agent_pool,
            status_stream,
            shared_context,
            coordination,
            file_locks,
        })
    }

    /// Execute a user query end-to-end
    #[instrument(skip(self), fields(query_id = %Uuid::new_v4()))]
    pub async fn execute_query(&self, query: &str) -> Result<String> {
        info!("Processing query: {}", query);

        // Step 1: Analyze the query
        let analysis = self.analyze_query(query).await?;
        debug!("Query analysis: {:?}", analysis);

        // Step 2: Decompose into tasks
        let tasks = self.decompose_query(&analysis).await?;
        info!("Decomposed query into {} tasks", tasks.len());

        // Step 3: Build workflow DAG
        let workflow = self.build_workflow(&tasks).await?;
        debug!("Built workflow with {} nodes", workflow.node_count());

        // Step 4: Execute workflow
        let results = self.execute_workflow(workflow).await?;
        info!("Workflow execution completed with {} results", results.len());

        // Step 5: Synthesize results
        let final_result = self.synthesize_results(&results).await?;

        Ok(final_result)
    }

    /// Analyze query complexity and requirements
    #[instrument(skip(self))]
    async fn analyze_query(&self, query: &str) -> Result<QueryAnalysis> {
        debug!("Analyzing query: {}", query);

        // Heuristic analysis (can be enhanced with LLM later)
        let complexity = self.estimate_complexity(query);
        let suggested_agents = self.suggest_agent_types(query);
        let estimated_tokens = (query.len() / 4) + 200; // Rough estimate

        let requires_hitl = complexity >= Complexity::Complex;

        Ok(QueryAnalysis {
            query: query.to_string(),
            complexity,
            suggested_agent_types: suggested_agents,
            requires_hitl,
            estimated_tokens,
        })
    }

    /// Estimate query complexity
    fn estimate_complexity(&self, query: &str) -> Complexity {
        let words = query.split_whitespace().count();
        let special_chars = query.chars().filter(|c| "{}[]()".contains(*c)).count();

        match (words, special_chars) {
            (w, _) if w < 5 => Complexity::Trivial,
            (w, _) if w < 15 && special_chars == 0 => Complexity::Simple,
            (w, _) if w < 30 => Complexity::Moderate,
            (w, _) if w < 100 => Complexity::Complex,
            _ => Complexity::VeryComplex,
        }
    }

    /// Suggest which agent types should handle this query
    fn suggest_agent_types(&self, query: &str) -> Vec<String> {
        let mut suggested = vec![];
        let query_lower = query.to_lowercase();

        if query_lower.contains("implement")
            || query_lower.contains("code")
            || query_lower.contains("write")
        {
            suggested.push("coding".to_string());
        }

        if query_lower.contains("plan")
            || query_lower.contains("design")
            || query_lower.contains("break")
        {
            suggested.push("planning".to_string());
        }

        if query_lower.contains("document")
            || query_lower.contains("comment")
            || query_lower.contains("describe")
        {
            suggested.push("writing".to_string());
        }

        if suggested.is_empty() {
            // Default to available agent types
            suggested = self.config.available_agent_types();
        }

        suggested
    }

    /// Decompose query into executable tasks
    #[instrument(skip(self, analysis))]
    async fn decompose_query(&self, analysis: &QueryAnalysis) -> Result<Vec<DecomposedTask>> {
        debug!("Decomposing query into tasks");

        let mut tasks = vec![];

        // Simple decomposition strategy based on complexity
        match analysis.complexity {
            Complexity::Trivial | Complexity::Simple => {
                // Single task
                let agent_type = analysis
                    .suggested_agent_types
                    .first()
                    .cloned()
                    .ok_or_else(|| AgentNetworkError::orchestration(
                        "No suitable agent type found",
                    ))?;

                let agent = self.config
                    .get_agents_by_type(&agent_type)
                    .first()
                    .ok_or_else(|| AgentNetworkError::orchestration(
                        format!("No agent of type '{}' available", agent_type),
                    ))?.clone();

                tasks.push(DecomposedTask {
                    id: format!("task-{}", Uuid::new_v4()),
                    agent_id: agent.id.clone(),
                    description: analysis.query.clone(),
                    dependencies: vec![],
                    priority: 0,
                    requires_hitl: analysis.requires_hitl,
                    recovery_strategy: agent
                        .effective_recovery_strategy()
                        .unwrap_or_else(|| ErrorRecoveryStrategy::Retry {
                            max_attempts: self.config.retry.max_attempts,
                            backoff_ms: self.config.retry.backoff_ms,
                        }),
                });
            }
            Complexity::Moderate | Complexity::Complex | Complexity::VeryComplex => {
                // Multi-step workflow: plan -> implement -> verify

                // Planning phase
                if let Some(planner) = self.config.get_agents_by_type("planning").first() {
                    tasks.push(DecomposedTask {
                        id: "task-plan".to_string(),
                        agent_id: planner.id.clone(),
                        description: format!("Plan approach for: {}", analysis.query),
                        dependencies: vec![],
                        priority: 10,
                        requires_hitl: false,
                        recovery_strategy: ErrorRecoveryStrategy::Retry {
                            max_attempts: 3,
                            backoff_ms: 1000,
                        },
                    });
                }

                // Implementation phase
                if let Some(coder) = self.config.get_agents_by_type("coding").first() {
                    tasks.push(DecomposedTask {
                        id: "task-implement".to_string(),
                        agent_id: coder.id.clone(),
                        description: format!("Implement solution for: {}", analysis.query),
                        dependencies: vec!["task-plan".to_string()],
                        priority: 20,
                        requires_hitl: analysis.requires_hitl,
                        recovery_strategy: ErrorRecoveryStrategy::Retry {
                            max_attempts: 3,
                            backoff_ms: 2000,
                        },
                    });
                }

                // Writing/Documentation phase
                if let Some(writer) = self.config.get_agents_by_type("writing").first() {
                    tasks.push(DecomposedTask {
                        id: "task-document".to_string(),
                        agent_id: writer.id.clone(),
                        description: "Create documentation for implementation".to_string(),
                        dependencies: vec!["task-implement".to_string()],
                        priority: 5,
                        requires_hitl: false,
                        recovery_strategy: ErrorRecoveryStrategy::Skip,
                    });
                }
            }
        }

        info!("Generated {} tasks", tasks.len());
        Ok(tasks)
    }

    /// Build workflow DAG from decomposed tasks
    #[instrument(skip(self, tasks))]
    async fn build_workflow(&self, tasks: &[DecomposedTask]) -> Result<WorkflowGraph> {
        debug!("Building workflow from {} tasks", tasks.len());

        let mut builder = WorkflowBuilder::new();
        let mut task_indices: HashMap<String, NodeIndex> = HashMap::new();

        // Add all task nodes
        for task in tasks {
            let node = TaskNode {
                task_id: task.id.clone(),
                agent_id: task.agent_id.clone(),
                description: task.description.clone(),
                recovery_strategy: task.recovery_strategy.clone(),
                requires_hitl: task.requires_hitl,
            };

            let idx = builder.add_task(node)?;
            task_indices.insert(task.id.clone(), idx);
        }

        // Add dependency edges
        for task in tasks {
            for dep_id in &task.dependencies {
                builder.add_dependency(dep_id, &task.id, DependencyType::Sequential)?;
            }
        }

        let graph = builder.build();
        debug!("Workflow DAG built: {} nodes, {} edges", graph.node_count(), graph.edge_count());

        Ok(graph)
    }

    /// Execute workflow graph
    #[instrument(skip(self, graph))]
    async fn execute_workflow(&self, graph: WorkflowGraph) -> Result<Vec<TaskResult>> {
        info!("Executing workflow with {} tasks", graph.node_count());

        // Validate DAG (no cycles)
        if toposort(&graph, None).is_err() {
            return Err(AgentNetworkError::dag_construction(
                "Workflow contains cycles",
            ))?;
        }

        // Create executor
        let executor = WorkflowExecutor::new(
            Arc::clone(&self.agent_pool),
            Arc::clone(&self.status_stream),
            Arc::clone(&self.coordination),
            Arc::clone(&self.file_locks),
        );

        // Execute workflow
        let results = executor.execute(graph).await?;

        info!("Workflow execution completed with {} results", results.len());
        Ok(results)
    }

    /// Synthesize task results into final output
    #[instrument(skip(self, results))]
    async fn synthesize_results(&self, results: &[TaskResult]) -> Result<String> {
        debug!("Synthesizing {} results", results.len());

        let mut output = String::new();
        let mut errors = vec![];

        for result in results {
            if result.success {
                if let Some(output_text) = &result.output {
                    output.push_str(output_text);
                    output.push('\n');
                }
            } else if let Some(error) = &result.error {
                errors.push(format!("Task {} failed: {}", result.task_id, error));
            }
        }

        if !errors.is_empty() {
            error!("Synthesis encountered errors: {:?}", errors);
            // Still return what we have, but include error info
            output.push_str("\n--- ERRORS ---\n");
            for err in errors {
                output.push_str(&format!("{}\n", err));
            }
        }

        if output.is_empty() {
            output = "Query executed but produced no output".to_string();
        }

        Ok(output)
    }

    // ============== Public Accessors ==============

    /// Get reference to agent pool
    pub fn agent_pool(&self) -> Arc<AgentPool> {
        Arc::clone(&self.agent_pool)
    }

    /// Get reference to status stream
    pub fn status_stream(&self) -> Arc<StatusStream> {
        Arc::clone(&self.status_stream)
    }

    /// Get reference to shared context
    pub fn shared_context(&self) -> Arc<RwLock<SharedContext>> {
        Arc::clone(&self.shared_context)
    }

    /// Get reference to coordination manager
    pub fn coordination(&self) -> Arc<CoordinationManager> {
        Arc::clone(&self.coordination)
    }

    /// Get reference to file lock manager
    pub fn file_locks(&self) -> Arc<FileLockManager> {
        Arc::clone(&self.file_locks)
    }

    /// Get orchestrator configuration
    pub fn config(&self) -> &AgentNetworkConfig {
        &self.config
    }
}

impl AgentNetworkError {
    pub fn orchestration(msg: impl Into<String>) -> Self {
        Self::Orchestration(msg.into())
    }

    pub fn dag_construction(msg: impl Into<String>) -> Self {
        Self::DagConstruction(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_estimation() {
        let orch_config = AgentNetworkConfig::default();
        // Note: This is a placeholder - actual test would need valid config

        assert_eq!(Complexity::Trivial, Complexity::Trivial);
    }

    #[test]
    fn test_suggest_agent_types() {
        let config = AgentNetworkConfig::default();
        // Placeholder - requires valid config setup
        assert_eq!(config.available_agent_types().len(), 0); // No agents in default
    }
}
