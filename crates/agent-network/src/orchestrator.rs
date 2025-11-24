//! Core orchestrator logic
//!
//! The Orchestrator is the central controller that:
//! - Analyzes user queries
//! - Decomposes tasks into sub-tasks
//! - Generates dynamic DAG workflows
//! - Manages agent execution
//! - Handles error recovery and HITL integration
use derive_more::Display;
use petgraph::dot::Dot;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument, span, warn, Level};
use crate::agents::planning::{SubtaskSpec, TaskDecompositionPlan};
use crate::error::{AgentNetworkError, AgentNetworkResult};
use crate::hitl::{AuditLogger, ConsoleApprovalHandler, DefaultApprovalQueue};
use crate::tools::{FilesystemTool, ToolRegistry};
use crate::workflow::{
    DependencyType, TaskNode, TaskResult, WorkflowBuilder, WorkflowExecutor, WorkflowGraph,
};
use crate::agents::{AgentContext, AgentPool};
use crate::status_stream::StatusStream;
use crate::sharedcontext::SharedContext;
use crate::coordination::CoordinationManager;
use crate::filelocks::FileLockManager;
use ai_agent_common::llm::EmbeddingClient;
use ai_agent_common::{AgentConfig, AgentNetworkConfig, AgentType, ConversationId, ErrorRecoveryStrategy, ProjectScope, SystemConfig};
use ai_agent_history::HistoryManager;
use ai_agent_rag::SmartMultiSourceRag;
use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use anyhow::{Context, Result};

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
    approval_queue: Arc<DefaultApprovalQueue>,
    audit_logger: Arc<AuditLogger>,

    rag: Arc<SmartMultiSourceRag>,
    history_manager: Arc<RwLock<HistoryManager>>,
    embedding_client: Arc<EmbeddingClient>,
    tool_registry: Arc<ToolRegistry>
}

// Context for the decomposition step to be  better
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    /// Project name and description
    pub project_name: String,
    pub description: Option<String>,

    /// Primary programming languages
    pub languages: Vec<String>,

    /// Main frameworks/technologies
    pub frameworks: Vec<String>,

    /// Project structure overview
    pub directory_structure: String,  // e.g., "src/, tests/, docs/"

    /// Key files and their purposes
    pub key_files: Vec<KeyFile>,

    /// Active development areas
    pub active_areas: Vec<String>,

    /// Known constraints or requirements
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyFile {
    pub path: String,
    pub purpose: String,  // e.g., "Main entry point", "Core business logic"
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DecompositionInput {
    /// The user's query/goal
    pub query: String,

    /// Analyzed complexity and metadata
    pub analysis: QueryAnalysis,

    /// Available agent types and their capabilities
    pub available_agents: Vec<AgentCapability>,

    /// Project context (files, dependencies, etc.)
    pub project_context: Option<String>,

    /// RAG-retrieved relevant examples
    pub example_decompositions: Option<Vec<String>>,
}

/// Represents a decomposed task for a single agent
#[derive(Debug, Clone, Display)]
#[display("TaskID: {}, AgentID: {}, Description: {}, Dependencies: {:?}, HITL: {}, recovery_strategy: {}", id, agent_id, description, dependencies, requires_hitl, recovery_strategy)]
pub struct DecomposedTask {
    pub id: String,
    pub agent_id: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub requires_hitl: bool,
    pub recovery_strategy: ErrorRecoveryStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Display)]
#[display("AgentType: {}, Description: {}, Capabilities:{:?}", agent_type, description,capabilities)]
pub struct AgentCapability {
    pub agent_type: AgentType,
    pub description: String,
    pub capabilities: Vec<String>,
}

/// Query analysis result
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Display)]
#[display("Query: {}, Complexity: {}, HITL:{}, Estimated Tokens:{}",query, complexity, requires_hitl, estimated_tokens)]
pub struct QueryAnalysis {
    pub query: String,
    pub complexity: Complexity,
    pub requires_hitl: bool,
    pub estimated_tokens: usize,
}

/// Complexity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, Display)]
pub enum Complexity {
    Trivial = 0,
    Simple = 1,
    Moderate = 2,
    Complex = 3,
    VeryComplex = 4,
}

impl Orchestrator {
    #[instrument(name = "orchestrator_initialization", skip(config), fields(agents = config.agent_network.agents.len()))]
    /// Create a new Orchestrator
    pub async fn new(config: SystemConfig) -> Result<Self> {
        let span = span!(Level::INFO, "orchestrator.init");
        let _enter = span.enter();

        info!("Initializing Orchestrator");
        debug!("Loading {} agents", config.agent_network.agents.len());

        let agent_pool = Arc::new(AgentPool::new(&config.agent_network.agents).await?);
        let status_stream = Arc::new(StatusStream::new());
        let shared_context = Arc::new(RwLock::new(SharedContext::new()));
        let coordination = Arc::new(CoordinationManager::new());
        let file_locks = Arc::new(FileLockManager::new(30));

        info!("Orchestrator initialized successfully");

        let hitl_mode = config.agent_network.hitl.mode;
        let risk_threshold = config.agent_network.hitl.risk_threshold;
        let approval_queue = Arc::new(DefaultApprovalQueue::new(hitl_mode, risk_threshold));
        let audit_logger = Arc::new(crate::hitl::AuditLogger);

        // Spawn approval handler background task
        let queue_clone = Arc::clone(&approval_queue);
        let handler = Arc::new(ConsoleApprovalHandler);
        tokio::spawn(async move {
            queue_clone.run_approver(handler).await;
        });
        let embedding_client = Arc::new(EmbeddingClient::new(&config.embedding.dense_model, config.embedding.vector_size)?);

        let rag = SmartMultiSourceRag::new(&config,embedding_client.clone()).await?;
    // Initialize HistoryManager (if Postgres configured)
        let history_manager = Arc::new(RwLock::new(HistoryManager::new(&config.storage.postgres_url, &config.rag).await?));
        let tool_registry = Arc::new(ToolRegistry::new());

        info!("Orchestrator initialized successfully");
        Ok(Self {
            config:config.agent_network,
            agent_pool,
            status_stream,
            shared_context,
            coordination,
            file_locks,
            approval_queue,
            audit_logger,
            history_manager,
            rag,
            embedding_client,
            tool_registry
        })
    }

    /// Execute a user query end-to-end
    #[instrument(name = "query_execution", skip(self), fields(query_id = %Uuid::new_v4()))]
    pub async fn execute_query(&self,
        query: &str,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
        ) -> Result<String> {
        info!("Processing query: {}", query);

        // Step 1: Analyze the query
        let analysis = self.analyze_query(query).await?;
        debug!("Query analysis: {:?}", analysis);

        // Step 2: Decompose into tasks (or route directly for simple tasks)
        let tasks = match analysis.complexity {
            Complexity::Trivial | Complexity::Simple => {
                info!("Simple task detected, routing directly to appropriate agent");
                self.route_to_single_agent(&analysis, &project_scope, &conversation_id).await?
            },
            _ => {
                info!("Complex task detected, using planning agent decomposition");
                self.decompose_query(&project_scope, &conversation_id, &analysis).await?
            }
        };
        info!("Generated {} tasks", tasks.len());

        // Step 3: Build workflow DAG
        let workflow = self.build_workflow(&tasks).await?;
        debug!("Built workflow with {} nodes", workflow.node_count());

        // Step 4: Execute workflow
        let results = self.execute_workflow(workflow, project_scope, conversation_id).await?;
        info!("Workflow execution completed with {} results", results.len());

        // Step 5: Synthesize results
        let final_result = self.synthesize_results(&results).await?;

        Ok(final_result)
    }

    /// Analyze query complexity and requirements
    #[instrument(name = "query_analysis", skip(self))]
    async fn analyze_query(&self, query: &str) -> Result<QueryAnalysis> {
        debug!("Analyzing query: {}", query);

        // Heuristic analysis (can be enhanced with LLM later)
        let complexity = self.estimate_complexity(query);
        let estimated_tokens = (query.len() / 4) + 200; // Rough estimate

        let requires_hitl = complexity >= Complexity::Complex;

        Ok(QueryAnalysis {
            query: query.to_string(),
            complexity,
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

    /// Route simple tasks directly to appropriate agent without planning
    #[instrument(name = "single_agent_routing", skip(self, analysis, project_scope, conversation_id))]
    async fn route_to_single_agent(&self,
        analysis: &QueryAnalysis,
        project_scope: &ProjectScope,
        conversation_id: &ConversationId) -> Result<Vec<DecomposedTask>> {
        
        debug!("Routing simple task directly to agent");

        // Get available agent types and their capabilities
        let available_agents: Vec<AgentCapability> = self.agent_pool
            .list_agent_ids()
            .iter()
            .map(|agent_id| {
                let agent = self.agent_pool.get_agent(agent_id).unwrap();
                AgentCapability {
                    agent_type: agent.agent_type(),
                    description: format!("{} agent", agent.system_prompt()),
                    capabilities: vec![],
                }
            })
            .filter(|agent| agent.agent_type != AgentType::Planning) // exclude planning
            .collect();

        // Use a small model to classify which agent should handle this task
        let selected_agent_type = self.classify_task_agent(&analysis.query, &available_agents).await?;
        
        // Get the actual agent instance
        let agent = self.agent_pool
            .get_agent_by_type(selected_agent_type)
            .ok_or_else(|| AgentNetworkError::orchestration(
                format!("No agent of type '{}' available", selected_agent_type)
            ))?;

        // Create a single task
        let task_id = format!("{}-{}", selected_agent_type, Uuid::new_v4());
        let task = DecomposedTask {
            id: task_id,
            agent_id: agent.id().to_string(),
            description: analysis.query.clone(),
            dependencies: vec![],
            recovery_strategy: ErrorRecoveryStrategy::Skip,
            requires_hitl: analysis.requires_hitl,
        };

        Ok(vec![task])
    }

    /// Use a lightweight model to classify which agent type should handle the task
    async fn classify_task_agent(&self, query: &str, available_agents: &[AgentCapability]) -> Result<AgentType> {
        // Create agent type descriptions for classification
        let agent_descriptions: Vec<String> = available_agents
            .iter()
            .map(|agent| format!("{}: {}", agent.agent_type, agent.description))
            .collect();

        // Simple classification prompt
        let classification_prompt = format!(
            r#"Given this task: "{}"

Available agents:
{}

Which single agent type should handle this task? Consider:
- Coding tasks -> Coding
- Writing/documentation tasks -> Writing  
- Quality review/evaluation tasks -> Evaluator

Return only the agent type name (e.g., "Coding", "Writing", "Evaluator")."#,
            query,
            agent_descriptions.join("\n")
        );

        // For now, use simple heuristics (can be enhanced with LLM later)
        let query_lower = query.to_lowercase();
        
        // Simple keyword-based classification
        if query_lower.contains("write") || query_lower.contains("implement") || 
           query_lower.contains("create") || query_lower.contains("function") ||
           query_lower.contains("code") || query_lower.contains("script") {
            Ok(AgentType::Coding)
        } else if query_lower.contains("document") || query_lower.contains("readme") ||
                  query_lower.contains("explain") || query_lower.contains("describe") {
            Ok(AgentType::Writing)
        } else if query_lower.contains("review") || query_lower.contains("evaluate") ||
                  query_lower.contains("assess") || query_lower.contains("check") {
            Ok(AgentType::Evaluator)
        } else {
            // Default to Coding for ambiguous cases
            Ok(AgentType::Coding)
        }
    }


    /// LLM-driven task decomposition using Planning Agent
    #[instrument(name = "query_decomposition", skip(self, analysis, project_scope, conversation_id))]
    async fn decompose_query(&self,
        project_scope: &ProjectScope,
        conversation_id: &ConversationId,
        analysis: &QueryAnalysis) -> Result<Vec<DecomposedTask>> {
        info!("Decomposing query using LLM planning agent: {}", analysis);

        // Get planning agent from pool
        let planning_agent = self.agent_pool
            .get_agent_by_type(AgentType::Planning)
            .ok_or_else(|| AgentNetworkError::orchestration("No planning agent available"))?;

        // Prepare decomposition input
        let available_agents: Vec<AgentCapability> = self.agent_pool
            .list_agent_ids()
            .iter()
            .map(|agent_id| {
                let agent = self.agent_pool.get_agent(agent_id).unwrap();
                AgentCapability {
                    agent_type: agent.agent_type(),
                    description: format!("{} agent", agent.system_prompt()),
                    capabilities: vec![/* agent-specific capabilities */],
                }
            })
            .filter(|agent| agent.agent_type != AgentType::Planning) // dont use planning tasks
            .collect();

        info!("Available agents: {:?}", available_agents);

        let decomposition_input = DecompositionInput {
            query: analysis.query.clone(),
            analysis: analysis.clone(),
            available_agents,
            project_context: None, // TODO: add ProjectContext
            example_decompositions: None, // Could add RAG examples here
        };

        let description = format!(
                "Generate a task decomposition plan with a list of Subtasks for the following task:\n\n{}",
                serde_json::to_string_pretty(&decomposition_input)?
            );

        //TODO: populate with tools
        let fs_tool = FilesystemTool::new(&project_scope.root);
        self.tool_registry.register(Arc::new(fs_tool));

        // Get the tools info after registration
        let tools_info = self.tool_registry.get_tools_info();

        // Build agent context for planning with tools included
        let planning_context = AgentContext::new(
            planning_agent.id().into(),
            AgentType::Planning,
            format!("{}-{}",planning_agent.agent_type(), Uuid::new_v4()),
            description,
            project_scope.clone(),
            conversation_id.clone()
        ).with_tools(tools_info);

        info!("Planning Context: {}",planning_context);

        // Execute planning agent with extractor for structured output
        let result = planning_agent.execute(planning_context, self.tool_registry.clone()).await?;

        // Extract the structured plan
        let plan: TaskDecompositionPlan = result.extract()?;

        info!(
            "LLM generated {} subtasks with reasoning: {}",
            plan.subtasks.len(),
            plan.reasoning
        );
        // After extracting the plan from LLM:
        let plan: TaskDecompositionPlan = result.extract()
            .context("Failed to extract task decomposition plan")?;

        // Step 1: Create mapping from LLM task IDs to actual UUIDs
        let mut id_mapping: HashMap<String, String> = HashMap::new();

        // Step 2: First pass - generate UUIDs and build mapping
        let task_specs_with_ids: Vec<(String, SubtaskSpec)> = plan.subtasks
            .into_iter()
            .map(|subtask| {
                let actual_task_id = format!("{}-{}", subtask.agent_type, Uuid::new_v4());

                // Map LLM's ID to our generated UUID
                id_mapping.insert(subtask.id.clone(), actual_task_id.clone());

                (actual_task_id, subtask)
            })
            .collect();

        info!("ID Mapping: {:?}", id_mapping);

        // Step 3: Second pass - create tasks with resolved dependencies
        let tasks: Vec<DecomposedTask> = task_specs_with_ids
            .into_iter()
            .map(|(actual_task_id, subtask)| {
                let agent = self.config
                    .get_agents_by_type(subtask.agent_type)
                    .first()
                    .ok_or_else(|| AgentNetworkError::orchestration(
                        format!("No agent of type '{}' available", subtask.agent_type)
                    ))?
                    .clone();

                // Resolve dependencies: convert LLM IDs to actual UUIDs
                let resolved_dependencies: Vec<String> = subtask.dependencies
                    .iter()
                    .filter_map(|llm_dep_id| {
                        id_mapping.get(llm_dep_id).cloned().or_else(|| {
                            warn!("Could not resolve dependency '{}' for task '{}'", llm_dep_id, actual_task_id);
                            None
                        })
                    })
                    .collect();

                info!(
                    "Task '{}': LLM deps {:?} → Resolved deps {:?}",
                    actual_task_id, subtask.dependencies, resolved_dependencies
                );

                Ok(DecomposedTask {
                    id: actual_task_id,
                    agent_id: agent.id.clone(),
                    description: subtask.description,
                    dependencies: resolved_dependencies,  // Use resolved deps
                    requires_hitl: subtask.requires_approval || plan.requires_hitl,
                    recovery_strategy:  agent.effective_recovery_strategy()
                            .unwrap_or(ErrorRecoveryStrategy::Retry {
                                max_attempts: 3,
                                backoff_ms: 1000,
                            }),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(tasks)
    }

    fn parse_recovery_strategy(&self, strategy_str: &str) -> Result<ErrorRecoveryStrategy> {
        match strategy_str.to_lowercase().as_str() {
            "retry" => Ok(ErrorRecoveryStrategy::Retry {
                max_attempts: 3,
                backoff_ms: 1000,
            }),
            "skip" => Ok(ErrorRecoveryStrategy::Skip),
            "halt" => Ok(ErrorRecoveryStrategy::Abort),
            _ => Err(anyhow::anyhow!("Unknown recovery strategy: {}", strategy_str)),
        }
    }

    /// Build workflow DAG from decomposed tasks
    #[instrument(name = "workflow_construction", skip(self, tasks))]
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
        for (idx, task ) in tasks.iter().enumerate() {
            info!(
                "Task {}: id='{}', agent_id='{}', dependencies={:?}",
                idx, task.id, task.agent_id, task.dependencies
            );
            // match found agent capabilities to task id
            for from_id in &task.dependencies {

                    builder.add_dependency(&from_id, &task.id, DependencyType::Sequential)?;
            }
        }

        let graph = builder.build();
        debug!("Workflow DAG built: {} nodes, {} edges", graph.node_count(), graph.edge_count());

        Ok(graph)
    }

    fn export_workflow_graph_dot(graph: &WorkflowGraph) -> () {
        let dot =format!("{}", Dot::with_config(graph, &[]));
        std::fs::write("graph.dot", dot).expect("Failed to write DOT file")
    }

    /// Execute workflow graph
    #[instrument(name = "workflow_orchestration", skip(self, graph), fields(nodes = graph.node_count()))]
    pub async fn execute_workflow(&self,
        graph: WorkflowGraph,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
        ) -> Result<Vec<TaskResult>> {
        info!("Starting workflow execution with {} nodes", graph.node_count());

        let executor = WorkflowExecutor::new(
            self.agent_pool.clone(),
            self.status_stream.clone(),
            self.coordination.clone(),
            self.file_locks.clone(),
        )
        .with_hitl(
            self.approval_queue.clone(),
            self.audit_logger.clone(),
        )
        // ADD:
        .with_context_provider(
            Some(self.rag.clone()),
            Some(self.history_manager.clone()),
        );

        let results = executor.execute_with_hitl(graph, self.approval_queue.clone(), self.audit_logger.clone(),project_scope, conversation_id, self.tool_registry.clone()).await?;

        info!("Workflow execution completed with {} results", results.len());
        Ok(results)
    }

    /// Synthesize task results into final output
    #[instrument(name = "result_synthesis", skip(self, results))]
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
