//! Stateless Orchestrator for multi-agent coordination
//!
//! The Orchestrator provides core business logic for:
//! - Query analysis and complexity estimation
//! - Task decomposition and routing
//! - Workflow generation and execution
//! - Result synthesis

use std::sync::Arc;
use std::collections::HashMap;
use petgraph::graph::NodeIndex;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn, error, instrument};

use crate::agents::{AgentContext, AgentPool};
use crate::sharedcontext::SharedContext;
use crate::coordination::CoordinationManager;
use crate::filelocks::FileLockManager;
use crate::hitl::{AuditLogger, DefaultApprovalQueue};
use crate::workflow::{WorkflowExecutor, WorkflowGraph, TaskResult, WorkflowBuilder, TaskNode, DependencyType};

use ai_agent_common::{
    ConversationId, ProjectScope, SystemConfig, StatusEvent, EventSource, EventType,
    AgentNetworkConfig, AgentType, ErrorRecoveryStrategy,
};
use chrono;
use ai_agent_history::HistoryManager;
use ai_agent_rag::SmartMultiSourceRag;
use ai_agent_common::llm::EmbeddingClient;

/// Core Orchestrator for multi-agent coordination (stateless)
pub struct Orchestrator;

// Basic types for orchestration

/// Query analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAnalysis {
    pub query: String,
    pub complexity: Complexity,
    pub requires_hitl: bool,
    pub estimated_tokens: usize,
}

/// Complexity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Complexity {
    Trivial = 0,
    Simple = 1,
    Moderate = 2,
    Complex = 3,
    VeryComplex = 4,
}

/// Decomposed task representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecomposedTask {
    pub id: String,
    pub agent_id: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub recovery_strategy: ErrorRecoveryStrategy,
    pub requires_hitl: bool,
}

/// Agent capability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapability {
    pub agent_type: AgentType,
    pub description: String,
    pub capabilities: Vec<String>,
}

impl Orchestrator {
    /// Execute a user query with stateless orchestration
    #[instrument(name = "query_execution", skip_all)]
    pub async fn execute_query(
        query: &str,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
        status_sender: Arc<broadcast::Sender<StatusEvent>>,
        config: Arc<SystemConfig>,
        agent_pool: Arc<AgentPool>,
        shared_context: Arc<RwLock<SharedContext>>,
        coordination: Arc<CoordinationManager>,
        file_locks: Arc<FileLockManager>,
        approval_queue: Arc<DefaultApprovalQueue>,
        audit_logger: Arc<AuditLogger>,
        rag: Arc<SmartMultiSourceRag>,
        history_manager: Arc<RwLock<HistoryManager>>,
        embedding_client: Arc<EmbeddingClient>,
    ) -> Result<String> {
        info!("Processing query: {}", query);
        
        let conversation_id_str = conversation_id.to_string();

        // Step 1: Analyze the query
        let analysis = Self::analyze_query(query, &config.agent_network).await?;
        debug!("Query analysis: {:?}", analysis);
        
        // Emit query analysis completed event
        let analysis_event = StatusEvent {
            execution_id: conversation_id_str.clone(),
            timestamp: chrono::Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::WorkflowStepCompleted { 
                step_name: format!("Query Analysis ({})", analysis.complexity as u8)
            },
        };
        
        if let Err(_) = status_sender.send(analysis_event) {
            debug!("No subscribers for query analysis event");
        }

        // Step 2: Decompose into tasks (or route directly for simple tasks)
        let tasks = match analysis.complexity {
            Complexity::Trivial | Complexity::Simple => {
                info!("Simple task detected, routing directly to appropriate agent");
                Self::route_to_single_agent(
                    &analysis, 
                    &project_scope, 
                    &conversation_id, 
                    &agent_pool, 
                    &config.agent_network
                ).await?
            },
            _ => {
                info!("Complex task detected, using planning agent decomposition");
                // For now, fallback to single agent until decomposition is implemented
                Self::route_to_single_agent(
                    &analysis, 
                    &project_scope, 
                    &conversation_id, 
                    &agent_pool, 
                    &config.agent_network
                ).await?
            }
        };
        info!("Generated {} tasks", tasks.len());
        
        // Emit task decomposition completed event
        let decomposition_event = StatusEvent {
            execution_id: conversation_id_str.clone(),
            timestamp: chrono::Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::WorkflowStepCompleted { 
                step_name: format!("Task Decomposition ({} tasks)", tasks.len())
            },
        };
        
        if let Err(_) = status_sender.send(decomposition_event) {
            debug!("No subscribers for task decomposition event");
        }

        // Step 3: Build workflow DAG
        let workflow = Self::build_workflow(&tasks).await?;
        debug!("Built workflow with {} nodes", workflow.node_count());
        
        // Emit workflow construction completed event
        let workflow_event = StatusEvent {
            execution_id: conversation_id_str.clone(),
            timestamp: chrono::Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::WorkflowStepCompleted { 
                step_name: format!("Workflow Construction ({} nodes)", workflow.node_count())
            },
        };
        
        if let Err(_) = status_sender.send(workflow_event) {
            debug!("No subscribers for workflow construction event");
        }

        // Step 4: Execute workflow
        let results = Self::execute_workflow(
            workflow, 
            project_scope, 
            conversation_id,
            agent_pool,
            coordination,
            file_locks,
            approval_queue,
            audit_logger,
            rag,
            history_manager,
            status_sender.clone(),
        ).await?;
        info!("Workflow execution completed with {} results", results.len());

        // Step 5: Synthesize results
        let final_result = Self::synthesize_results(&results).await?;
        
        // Emit result synthesis completed event
        let synthesis_event = StatusEvent {
            execution_id: conversation_id_str.clone(),
            timestamp: chrono::Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::WorkflowStepCompleted { 
                step_name: format!("Result Synthesis ({} chars)", final_result.len())
            },
        };
        
        if let Err(_) = status_sender.send(synthesis_event) {
            debug!("No subscribers for result synthesis event");
        }

        Ok(final_result)
    }

    /// Analyze query complexity and requirements
    #[instrument(name = "query_analysis", skip_all)]
    async fn analyze_query(query: &str, config: &AgentNetworkConfig) -> Result<QueryAnalysis> {
        debug!("Analyzing query: {}", query);

        // Heuristic analysis (can be enhanced with LLM later)
        let complexity = Self::estimate_complexity(query);
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
    fn estimate_complexity(query: &str) -> Complexity {
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
    #[instrument(name = "single_agent_routing", skip_all)]
    async fn route_to_single_agent(
        analysis: &QueryAnalysis,
        project_scope: &ProjectScope,
        conversation_id: &ConversationId,
        agent_pool: &Arc<AgentPool>,
        config: &AgentNetworkConfig,
    ) -> Result<Vec<DecomposedTask>> {
        debug!("Routing simple task directly to agent");

        // Use simple classification to determine agent type
        let selected_agent_type = Self::classify_task_agent(&analysis.query).await?;

        // Get the actual agent instance
        let agent = agent_pool
            .get_agent_by_type(selected_agent_type)
            .ok_or_else(|| anyhow::anyhow!(
                "No agent of type '{:?}' available", selected_agent_type
            ))?;

        // Create a single task
        let task_id = format!("{:?}-{}", selected_agent_type, Uuid::new_v4());
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

    /// Use simple heuristics to classify which agent type should handle the task
    async fn classify_task_agent(query: &str) -> Result<AgentType> {
        let query_lower = query.to_lowercase();

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

    /// Build workflow DAG from decomposed tasks
    async fn build_workflow(tasks: &[DecomposedTask]) -> Result<WorkflowGraph> {
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
        for (idx, task) in tasks.iter().enumerate() {
            info!(
                "Task {}: id='{}', agent_id='{}', dependencies={:?}",
                idx, task.id, task.agent_id, task.dependencies
            );
            
            // Add dependencies between tasks
            for from_id in &task.dependencies {
                builder.add_dependency(&from_id, &task.id, DependencyType::Sequential)?;
            }
        }

        let graph = builder.build();
        debug!("Workflow DAG built: {} nodes, {} edges", graph.node_count(), graph.edge_count());

        Ok(graph)
    }

    /// Execute workflow
    async fn execute_workflow(
        workflow: WorkflowGraph,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
        agent_pool: Arc<AgentPool>,
        coordination: Arc<CoordinationManager>,
        file_locks: Arc<FileLockManager>,
        approval_queue: Arc<DefaultApprovalQueue>,
        audit_logger: Arc<AuditLogger>,
        rag: Arc<SmartMultiSourceRag>,
        history_manager: Arc<RwLock<HistoryManager>>,
        status_sender: Arc<broadcast::Sender<StatusEvent>>,
    ) -> Result<Vec<TaskResult>> {
        debug!("Executing workflow with {} nodes", workflow.node_count());

        // Create executor
        let executor = WorkflowExecutor::new(
            agent_pool,
            coordination,
            file_locks,
        );
        
        // Execute the workflow with HITL
        let results = executor.execute_with_hitl(
            workflow,
            approval_queue,
            audit_logger, 
            project_scope, 
            conversation_id,
            status_sender,
        ).await?;
        
        Ok(results)
    }

    /// Synthesize task results into final output
    async fn synthesize_results(results: &[TaskResult]) -> Result<String> {
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
}