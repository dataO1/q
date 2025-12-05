//! Workflow execution engine with wave-based parallel execution
//!
//! Implements efficient parallel task execution using topological sorting
//! to organize tasks into waves, ensuring proper dependency satisfaction
//! while maximizing parallelism through concurrent task spawning.
use tracing::{info, debug, warn, error, instrument, span, Level, Instrument};
use crate::error::{AgentNetworkError, AgentNetworkResult};
use crate::hitl::{AuditEvent, AuditLogger, RiskAssessment};
use crate::workflow::{TaskNode, TaskResult, WorkflowGraph, DependencyType};
use crate::agents::{AgentPool, AgentContext};
use crate::tools::ToolSet;
use crate::coordination::CoordinationManager;
use crate::filelocks::FileLockManager;
use crate::execution_manager::BidirectionalEventChannel;
use ai_agent_common::{ConversationId, ProjectScope, StatusEvent, EventSource, EventType, ExecutionPlan, WaveInfo, TaskInfo};
use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, VecDeque, BTreeMap};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, broadcast};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Wave-based workflow executor for parallel task execution
pub struct WorkflowExecutor {
    /// Agent pool for task execution
    agent_pool: Arc<AgentPool>,


    /// Task coordination manager
    coordination: Arc<CoordinationManager>,

    /// File lock manager for concurrent access
    file_locks: Arc<FileLockManager>,

    /// Execution configuration
    config: ExecutorConfig,
    /// Context provider for RAG and history
    context_provider: Option<Arc<crate::rag::ContextProvider>>,
}

/// Executor configuration
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum concurrent tasks per wave
    pub max_concurrent_tasks: usize,

    /// Task execution timeout
    pub task_timeout: Duration,

    /// Enable detailed metrics collection
    pub collect_metrics: bool,

    /// Maximum retries per task
    pub max_retries: usize,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 16,
            task_timeout: Duration::from_secs(5000),
            collect_metrics: true,
            max_retries: 3,
        }
    }
}

/// Represents a wave of tasks that can execute in parallel
#[derive(Debug, Clone)]
pub struct ExecutionWave {
    pub wave_index: usize,
    pub task_indices: Vec<NodeIndex>,
    pub parallel_degree: usize,
}

/// Task execution metrics
#[derive(Debug, Clone)]
pub struct TaskMetrics {
    pub task_id: String,
    pub wave_index: usize,
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub duration: Option<Duration>,
    pub retries: usize,
    pub success: bool,
}

/// Execution statistics
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub total_waves: usize,
    pub total_duration: Duration,
    pub average_wave_duration: Duration,
    pub task_metrics: Vec<TaskMetrics>,
}

impl WorkflowExecutor {
    /// Create a new workflow executor
    pub fn new(
        agent_pool: Arc<AgentPool>,
        coordination: Arc<CoordinationManager>,
        file_locks: Arc<FileLockManager>,
    ) -> Self {
        Self::with_config(agent_pool, coordination, file_locks, ExecutorConfig::default())
    }


    pub fn with_context_provider<'a>(
        mut self,
        rag: Option<Arc<ai_agent_rag::SmartMultiSourceRag>>,
        history_manager: Option<Arc<tokio::sync::RwLock<ai_agent_history::manager::HistoryManager>>>,
    ) -> Self {
        if let (Some(rag), Some(history)) = (rag, history_manager) {
            let provider = crate::rag::ContextProvider::new(
                rag,
                history,
                8192, // Token budget
            );
            self.context_provider = Some(Arc::new(provider));
        }
        self
    }

    /// Create executor with custom configuration
    pub fn with_config(
        agent_pool: Arc<AgentPool>,
        coordination: Arc<CoordinationManager>,
        file_locks: Arc<FileLockManager>,
        config: ExecutorConfig,
    ) -> Self {
        Self {
            agent_pool,
            coordination,
            file_locks,
            config,
            context_provider: None
        }
    }

    /// Execute workflow with wave-based parallel execution
    #[instrument(name = "workflow_execution", skip(self, graph, event_channel), fields(task_count = %graph.node_count()))]
    pub async fn execute_with_hitl(&self,
            graph: WorkflowGraph,
            audit_logger: Arc<AuditLogger>,
            project_scope: ProjectScope,
            conversation_id: ConversationId,
            event_channel: BidirectionalEventChannel,
        ) -> AgentNetworkResult<Vec<TaskResult>> {
        let start_time = Instant::now();
        info!("Starting workflow execution: {} tasks", graph.node_count());

        // Validate DAG (no cycles)
        let sorted_nodes = toposort(&graph, None).map_err(|_| {
            AgentNetworkError::dag_construction("Workflow graph contains cycles")
        })?;

        if sorted_nodes.is_empty() {
            return Ok(vec![]);
        }

        debug!("Topological sort completed: {} nodes in order", sorted_nodes.len());

        // Compute execution waves
        let waves = self.compute_execution_waves(&graph, &sorted_nodes)?;
        info!("Computed {} execution waves", waves.len());

        // Create and send ExecutionPlan
        let execution_plan = self.create_execution_plan(&graph, &waves).await?;
        let execution_plan_event = StatusEvent {
            id: conversation_id.to_string(),
            timestamp: chrono::Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::ExecutionPlanReady {
                plan: execution_plan,
            },
        };

        if let Err(_) = event_channel.send(execution_plan_event).await {
            debug!("Failed to send execution plan event");
        }

        // Execute waves sequentially, tasks within waves in parallel
        let mut all_results: HashMap<String, TaskResult> = HashMap::new();

        let mut tmp = waves.iter().enumerate().into_iter();
        while let Some((_wave_idx, wave)) = tmp.next() {
            let wave_results = self.execute_wave_with_hitl(
                &graph,
                wave,
                &all_results,
                Arc::clone(&audit_logger),
                project_scope.clone(),
                conversation_id.clone(),
                event_channel.clone(),
            ).await?;

            for result in wave_results {
                all_results.insert(result.task_id.clone(), result);
            }
        }

        // Collect results in original order
        let results: Vec<TaskResult> = sorted_nodes
            .iter()
            .filter_map(|node_idx| {
                let task = &graph[*node_idx];
                all_results.get(&task.task_id).cloned()
            })
            .collect();

        let duration = start_time.elapsed();
        info!(
            "Workflow execution completed in {:?}: {} successful, {} failed",
            duration,
            results.iter().filter(|r| r.success).count(),
            results.iter().filter(|r| !r.success).count()
        );

        Ok(results)
    }

    /// Execute a single wave of tasks in parallel
    #[instrument(name = "wave_execution", skip(self, graph, wave, previous_results), fields(
        wave_index = wave.wave_index,
        tasks = wave.task_indices.len()
    ))]
    async fn execute_wave_with_hitl(
        &self,
        graph: &WorkflowGraph,
        wave: &ExecutionWave,
        previous_results: &HashMap<String, TaskResult>,
        audit_logger: Arc<AuditLogger>,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
        event_channel: BidirectionalEventChannel,
    ) -> AgentNetworkResult<Vec<TaskResult>> {
        debug!("Executing wave {}: {} parallel tasks", wave.wave_index, wave.task_indices.len());
        // Log wave information with structured fields
        debug!(
            wave_index = wave.wave_index,
            task_count = wave.task_indices.len(),
            "Starting wave execution"
        );

        // Emit wave started event
        let task_ids: Vec<String> = wave.task_indices.iter()
            .map(|&idx| graph[idx].task_id.clone())
            .collect();

        let wave_started_event = ai_agent_common::StatusEvent {
            id: conversation_id.to_string(),
            timestamp: chrono::Utc::now(),
            source: ai_agent_common::EventSource::Orchestrator,
            event: ai_agent_common::EventType::WaveStarted {
                wave_index: wave.wave_index,
                task_count: wave.task_indices.len(),
                task_ids,
            },
        };

        if let Err(_) = event_channel.send(wave_started_event).await {
            debug!("Failed to send wave started event");
        }

        let mut handles: Vec<JoinHandle<AgentNetworkResult<TaskResult>>> = vec![];

        // Spawn all tasks in the wave
        for task_idx in &wave.task_indices {
            let task = graph[*task_idx].clone();
            let agent_pool = Arc::clone(&self.agent_pool);
            let coordination = Arc::clone(&self.coordination);
            let file_locks = Arc::clone(&self.file_locks);
            let timeout = self.config.task_timeout;
            let max_retries = self.config.max_retries;
            let wave_index = wave.wave_index;
            let audit_logger_clone = Arc::clone(&audit_logger);
            let context_provider = self.context_provider.clone();

            let project_scope = project_scope.clone();
            let conversation_id = conversation_id.clone();
            let event_channel = event_channel.clone();

            let previous_results_clone = previous_results.clone();

            // Create a task-specific span that will be the parent for this task execution
            let task_span = tracing::info_span!(
                "task_execution",
                task_id = %task.task_id,
                agent_id = %task.agent_id,
                wave_index = wave_index
            );

            let handle = tokio::spawn(
                async move {
                    execute_task_with_retry(
                        task.clone(),
                        agent_pool,
                        coordination,
                        file_locks,
                        audit_logger_clone,
                        context_provider,
                        timeout,
                        max_retries,
                        wave_index,
                        project_scope,
                        conversation_id,
                        event_channel,
                        &previous_results_clone
                    )
                    .await
                }
                .instrument(task_span) // Propagate the span context to the spawned task
            );

            handles.push(handle);
        }

        // Collect results from all spawned tasks
        let mut wave_results = vec![];
        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => {
                    wave_results.push(result);
                }
                Ok(Err(e)) => {
                    error!("Task execution error: {}", e);
                    wave_results.push(TaskResult {
                        task_id: "unknown".to_string(),
                        success: false,
                        output: None,
                        error: Some(e.to_string()),
                        tool_executions: vec![],
                        agent_id: None,
                        task_description: None,
                        completed_at: Some(chrono::Utc::now()),
                    });
                }
                Err(e) => {
                    error!("Join error: {}", e);
                    wave_results.push(TaskResult {
                        task_id: "unknown".to_string(),
                        success: false,
                        output: None,
                        error: Some(format!("Join error: {}", e)),
                        tool_executions: vec![],
                        agent_id: None,
                        task_description: None,
                        completed_at: Some(chrono::Utc::now()),
                    });
                }
            }
        }
        info!("Wave {} completed", wave.wave_index);

        // Count successes and failures
        let success_count = wave_results.iter().filter(|r| r.success).count();
        let failure_count = wave_results.len() - success_count;

        // Emit wave completed event
        let wave_completed_event = ai_agent_common::StatusEvent {
            id: conversation_id.to_string(),
            timestamp: chrono::Utc::now(),
            source: ai_agent_common::EventSource::Orchestrator,
            event: ai_agent_common::EventType::WaveCompleted {
                wave_index: wave.wave_index,
                success_count,
                failure_count,
            },
        };

        if let Err(_) = event_channel.send(wave_completed_event).await {
            debug!("Failed to send wave completed event");
        }

        Ok(wave_results)
    }

    /// Compute execution waves from topologically sorted nodes
    fn compute_execution_waves(
        &self,
        graph: &WorkflowGraph,
        sorted_nodes: &[NodeIndex],
    ) -> AgentNetworkResult<Vec<ExecutionWave>> {
        let mut waves = vec![];
        let mut processed = std::collections::HashSet::new();
        let mut wave_index = 0;

        while processed.len() < sorted_nodes.len() {
            let mut wave_tasks = vec![];

            // Find all nodes whose dependencies are already processed
            for node_idx in sorted_nodes {
                if processed.contains(node_idx) {
                    continue;
                }

                // Check if all dependencies are processed
                let mut all_deps_satisfied = true;
                for edge_idx in graph.edges_directed(*node_idx, petgraph::Direction::Incoming) {
                    let source = edge_idx.source();
                    if !processed.contains(&source) {
                        all_deps_satisfied = false;
                        break;
                    }
                }

                if all_deps_satisfied {
                    wave_tasks.push(*node_idx);

                    // Respect max concurrent limit per wave
                    if wave_tasks.len() >= self.config.max_concurrent_tasks {
                        break;
                    }
                }
            }

            if wave_tasks.is_empty() {
                break;
            }

            // Mark all tasks in this wave as processed
            for &task_idx in &wave_tasks {
                processed.insert(task_idx);
            }

            waves.push(ExecutionWave {
                wave_index,
                task_indices: wave_tasks,
                parallel_degree: self.config.max_concurrent_tasks,
            });

            wave_index += 1;
        }

        Ok(waves)
    }

    /// Create execution plan from computed waves
    async fn create_execution_plan(
        &self,
        graph: &WorkflowGraph,
        waves: &[ExecutionWave],
    ) -> AgentNetworkResult<ExecutionPlan> {
        let mut plan_waves = vec![];

        for wave in waves {
            let mut wave_tasks = vec![];

            for &node_idx in &wave.task_indices {
                let task_node = &graph[node_idx];

                // Get agent to determine agent_type
                let agent = self.agent_pool
                    .get_agent(&task_node.agent_id)
                    .ok_or_else(|| AgentNetworkError::Agent(format!("Agent not found: {}", task_node.agent_id)))?;

                // Get dependencies by checking incoming edges
                let mut dependencies = vec![];
                for edge_idx in graph.edges_directed(node_idx, petgraph::Direction::Incoming) {
                    let source_node = &graph[edge_idx.source()];
                    dependencies.push(source_node.task_id.clone());
                }

                // Get actual workflow steps from agent
                let steps = {
                    // Create a temporary agent context to get workflow steps
                    let temp_context = crate::agents::AgentContext::new(
                        task_node.description.clone(),
                        "temp".to_string(),
                        Some(task_node.task_id.clone())
                    );

                    agent.define_workflow_steps(&temp_context)
                        .iter()
                        .map(|step| step.name.clone())
                        .collect::<Vec<String>>()
                };

                wave_tasks.push(TaskInfo {
                    task_id: task_node.task_id.clone(),
                    agent_id: task_node.agent_id.clone(),
                    agent_type: format!("{:?}", agent.agent_type()),
                    description: task_node.description.clone(),
                    dependencies,
                    steps,
                });
            }

            plan_waves.push(WaveInfo {
                wave_index: wave.wave_index,
                tasks: wave_tasks,
            });
        }

        Ok(ExecutionPlan { waves: plan_waves })
    }

    /// Get executor configuration
    pub fn config(&self) -> &ExecutorConfig {
        &self.config
    }


}


/// Execute a single task
#[instrument(name = "task_execution", skip(agent_pool, audit_logger, context_provider, file_locks, previous_results), fields(
    task_id = %task.task_id,
    agent_id = %task.agent_id,
    description = %task.description
))]
async fn execute_single_task(
    task: TaskNode,
    agent_pool: Arc<AgentPool>,
    audit_logger: Arc<AuditLogger>,
    context_provider: Option<Arc<crate::rag::ContextProvider>>,
    file_locks: Arc<FileLockManager>,
    project_scope: ProjectScope,
    conversation_id: ConversationId,
    event_channel: BidirectionalEventChannel,
    previous_results: &HashMap<String, TaskResult>
) -> AgentNetworkResult<TaskResult> {
    // Get agent
    let agent = agent_pool
        .get_agent(&task.agent_id)
        .ok_or_else(|| AgentNetworkError::Agent(format!("Agent not found: {}", task.agent_id)))?;

    // Add agent type to current span
    let current_span = tracing::Span::current();
    current_span.record("agent_type", &format!("{:?}", agent.agent_type()));

    let mut agent_context = AgentContext::new(
        task.description.clone(),
        conversation_id.to_string(),
        Some(task.task_id.clone())
    ).with_project_scope(project_scope.clone());

    // Build dependency outputs from previous results
    let mut dependency_outputs = HashMap::new();
    for (task_id, task_result) in previous_results.iter() {
        if task_result.success {
            // Create a structured dependency output including both the agent output and tool executions
            let mut dep_output = serde_json::Map::new();

            // Add the agent's structured output
            if let Some(output) = &task_result.output {
                if let Ok(parsed_output) = serde_json::from_str::<serde_json::Value>(output) {
                    dep_output.insert("agent_output".to_string(), parsed_output);
                }
            }

            // Add tool executions
            let tool_executions_json = serde_json::to_value(&task_result.tool_executions)?;
            dep_output.insert("tool_executions".to_string(), tool_executions_json);

            // Add attribution metadata
            if let Some(agent_id) = &task_result.agent_id {
                dep_output.insert("agent_id".to_string(), serde_json::Value::String(agent_id.clone()));
            }
            if let Some(task_description) = &task_result.task_description {
                dep_output.insert("task_description".to_string(), serde_json::Value::String(task_description.clone()));
            }
            if let Some(completed_at) = &task_result.completed_at {
                dep_output.insert("completed_at".to_string(), serde_json::Value::String(completed_at.to_rfc3339()));
            }

            dependency_outputs.insert(task_id.clone(), serde_json::Value::Object(dep_output));
        }
    }

    if !dependency_outputs.is_empty() {
        let num_deps = dependency_outputs.len();
        agent_context = agent_context.with_dependency_outputs(dependency_outputs);
        debug!("Added {} dependency outputs to agent context", num_deps);
    }

    // Retrieve and inject context if provider available
    if let Some(provider) = context_provider.as_ref() {
        let context_retrieval_future = async {
            // Use task.description as refined query for RAG/History
            match provider.retrieve_context(task.description.clone(), project_scope, conversation_id.clone()).await {
                Ok(context) => {
                    if !context.is_empty() {
                        let context_length = context.len();
                        info!("Retrieved RAG+History context (length: {} chars)", context_length);
                        Some(context)
                    } else {
                        debug!("No RAG+History context retrieved");
                        None
                    }
                }
                Err(e) => {
                    warn!("Failed to retrieve context: {}", e);
                    // Continue without context
                    None
                }
            }
        };

        // Execute context retrieval with proper span instrumentation and apply result
        let context_result = context_retrieval_future
            .instrument(tracing::info_span!("rag_retrieval", query_length = task.description.len()))
            .await;

        if let Some(context) = context_result {
            agent_context = agent_context.with_rag_context(context);
            info!("Injected RAG+History context into agent");
        }
    }

    // Execute agent
    info!("Starting agent execution");
    let hitl_audit_logger = Some(audit_logger.clone());
    match agent.execute(agent_context, event_channel, hitl_audit_logger).await {
        Ok(result) => {
            info!("Agent execution completed successfully (tool executions: {})", result.tool_executions.len());

            // Store exchange in history if provider available
            if let Some(provider) = context_provider.as_ref() {
                if let Err(e) = provider.store_exchange(
                    task.description.clone(),
                    serde_json::to_string(&result.output)?,
                    conversation_id
                ).await {
                    warn!("Failed to store exchange in history: {}", e);
                }
            }
            Ok(TaskResult {
                task_id: task.task_id.clone(),
                success: true,
                output: Some(serde_json::to_string(&result.output)?),
                error: None,
                tool_executions: result.tool_executions,
                agent_id: Some(agent.id().to_string()),
                task_description: Some(task.description),
                completed_at: Some(chrono::Utc::now()),
            })
        }
        Err(e) => {
            Err(AgentNetworkError::AgentExecutionFailed{agent_id: agent.id().to_string(), reason:e.to_string()})
        }
    }
}

#[instrument(name = "task_retry_execution", skip(task, agent_pool, coordination, file_locks, audit_logger, context_provider, event_channel, previous_results), fields(
    task_id = %task.task_id,
    agent_id = %task.agent_id,
))]
/// Execute a single task with retry logic
async fn execute_task_with_retry(
    task: TaskNode,
    agent_pool: Arc<AgentPool>,
    coordination: Arc<CoordinationManager>,
    file_locks: Arc<FileLockManager>,
    audit_logger: Arc<AuditLogger>,
    context_provider: Option<Arc<crate::rag::ContextProvider>>,
    timeout: Duration,
    max_retries: usize,
    wave_index: usize,
    project_scope: ProjectScope,
    conversation_id: ConversationId,
    event_channel: BidirectionalEventChannel,
    previous_results: &HashMap<String, TaskResult>
) -> AgentNetworkResult<TaskResult> {

    // Log task start with structured fields
    info!(
        task_id = %task.task_id,
        agent_id = %task.agent_id,
        "Task starting"
    );

    let task_id = task.task_id.clone();
    let agent_id = task.agent_id.clone();

    // Register task
    coordination.register_task(task_id.clone(), agent_id.clone()).await?;

    // Emit task node started event
    let task_started_event = ai_agent_common::StatusEvent {
        id: conversation_id.to_string(),
        timestamp: chrono::Utc::now(),
        source: ai_agent_common::EventSource::Orchestrator,
        event: ai_agent_common::EventType::TaskNodeStarted {
            task_id: task_id.clone(),
            agent_id: agent_id.clone(),
            wave_index,
            description: task.description.clone(),
        },
    };

    if let Err(_) = event_channel.send(task_started_event).await {
        debug!("Failed to send task started event");
    }

    let mut retries = 0;
    let mut last_error = None;

    loop {
        // Execute task with timeout
        let result = tokio::time::timeout(timeout, execute_single_task(
            task.clone(),
            Arc::clone(&agent_pool),
            audit_logger.clone(),
            context_provider.clone(),
            Arc::clone(&file_locks),
            project_scope.clone(),
            conversation_id.clone(),
            event_channel.clone(),
            previous_results
        ))
        .await;

        match result {
            Ok(Ok(task_result)) => {
                // Success
                coordination
                    .update_task_status(&task_id, crate::coordination::TaskStatus::Completed)
                    .await?;

                // ADD HITL CHECK HERE:
                if let (logger) = (audit_logger.as_ref()) {
                    if task_result.success {
                        // Get agent for type information
                        if let Some(agent) = agent_pool.get_agent(&task.agent_id) {

                            let agent_result = crate::agents::AgentResult::from_string(
                                &task.agent_id.clone(),
                                task_result.clone().output.unwrap_or_default().as_ref(),
                            )?
                            .with_confidence(0.8); // Should come from actual agent result

                            let assessment = RiskAssessment::new(
                                &agent_result,
                                agent.agent_type(),
                                Some(format!("Task {} completed", task.task_id)),
                            );
                        }
                    }
                }

                // Emit task node completed event
                let task_completed_event = ai_agent_common::StatusEvent {
                    id: conversation_id.to_string(),
                    timestamp: chrono::Utc::now(),
                    source: ai_agent_common::EventSource::Orchestrator,
                    event: ai_agent_common::EventType::TaskNodeCompleted {
                        task_id: task_id.clone(),
                        agent_id: agent_id.clone(),
                        wave_index,
                        success: true,
                    },
                };

                if let Err(_) = event_channel.send(task_completed_event).await {
                    debug!("Failed to send task completed event");
                }

                return Ok(task_result);
            }
            Ok(Err(e)) => {
                // Execution error
                last_error = Some(e);
                if retries < max_retries && task.recovery_strategy.is_retryable() {
                    retries += 1;
                    warn!("Task {} failed, retry {} of {}", task_id, retries, max_retries);

                    // Exponential backoff
                    let backoff = Duration::from_millis(100 * 2_u64.pow(retries as u32 - 1));
                    tokio::time::sleep(backoff).await;
                } else {
                    break;
                }
            }
            Err(_) => {
                // Timeout
                last_error = Some(AgentNetworkError::Timeout {
                    operation: format!("Task {}", task_id),
                });
                if retries < max_retries {
                    retries += 1;
                    warn!("Task {} timed out, retry {} of {}", task_id, retries, max_retries);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else {
                    break;
                }
            }
        }
    }

    // All retries exhausted
    let error_msg = last_error.as_ref().map(|e| e.to_string()).unwrap_or_else(|| "Unknown error".to_string());

    coordination
        .update_task_status(&task_id, crate::coordination::TaskStatus::Failed)
        .await?;

    // Emit task node completed event for failed task
    let task_completed_event = ai_agent_common::StatusEvent {
        id: conversation_id.to_string(),
        timestamp: chrono::Utc::now(),
        source: ai_agent_common::EventSource::Orchestrator,
        event: ai_agent_common::EventType::TaskNodeCompleted {
            task_id: task_id.clone(),
            agent_id: agent_id.clone(),
            wave_index,
            success: false,
        },
    };

    if let Err(_) = event_channel.send(task_completed_event).await {
        debug!("Failed to send task completed event");
    }

    debug!("Task completed with failure");
    Ok(TaskResult {
        task_id,
        success: false,
        output: None,
        error: Some(error_msg),
        tool_executions: vec![],
        agent_id: Some(agent_id),
        task_description: Some(task.description.clone()),
        completed_at: Some(chrono::Utc::now()),
    })

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_config_defaults() {
        let config = ExecutorConfig::default();
        assert_eq!(config.max_concurrent_tasks, 16);
        assert_eq!(config.task_timeout, Duration::from_secs(300));
        assert!(config.collect_metrics);
    }

    #[test]
    fn test_execution_wave_creation() {
        let wave = ExecutionWave {
            wave_index: 0,
            task_indices: vec![],
            parallel_degree: 4,
        };
        assert_eq!(wave.wave_index, 0);
        assert_eq!(wave.parallel_degree, 4);
    }
}
