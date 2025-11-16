//! Workflow execution engine with wave-based parallel execution
//!
//! Implements efficient parallel task execution using topological sorting
//! to organize tasks into waves, ensuring proper dependency satisfaction
//! while maximizing parallelism through concurrent task spawning.

use crate::error::{AgentNetworkError, AgentNetworkResult};
use crate::workflow::{TaskNode, TaskResult, WorkflowGraph, DependencyType};
use crate::agents::{AgentPool, AgentContext};
use crate::status_stream::StatusStream;
use crate::coordination::CoordinationManager;
use crate::filelocks::FileLockManager;
use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, VecDeque, BTreeMap};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn, error, instrument};
use uuid::Uuid;

/// Wave-based workflow executor for parallel task execution
pub struct WorkflowExecutor {
    /// Agent pool for task execution
    agent_pool: Arc<AgentPool>,

    /// Status stream for event broadcasting
    status_stream: Arc<StatusStream>,

    /// Task coordination manager
    coordination: Arc<CoordinationManager>,

    /// File lock manager for concurrent access
    file_locks: Arc<FileLockManager>,

    /// Execution configuration
    config: ExecutorConfig,
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
            task_timeout: Duration::from_secs(300),
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
        status_stream: Arc<StatusStream>,
        coordination: Arc<CoordinationManager>,
        file_locks: Arc<FileLockManager>,
    ) -> Self {
        Self::with_config(agent_pool, status_stream, coordination, file_locks, ExecutorConfig::default())
    }

    /// Create executor with custom configuration
    pub fn with_config(
        agent_pool: Arc<AgentPool>,
        status_stream: Arc<StatusStream>,
        coordination: Arc<CoordinationManager>,
        file_locks: Arc<FileLockManager>,
        config: ExecutorConfig,
    ) -> Self {
        Self {
            agent_pool,
            status_stream,
            coordination,
            file_locks,
            config,
        }
    }

    /// Execute workflow with wave-based parallel execution
    #[instrument(skip(self, graph), fields(task_count = %graph.node_count()))]
    pub async fn execute(&self, graph: WorkflowGraph) -> AgentNetworkResult<Vec<TaskResult>> {
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

        // Execute waves sequentially, tasks within waves in parallel
        let mut all_results: HashMap<String, TaskResult> = HashMap::new();

        for (wave_idx, wave) in waves.iter().enumerate() {
            info!("Executing wave {} with {} tasks", wave_idx, wave.task_indices.len());

            let wave_results = self.execute_wave(&graph, wave, &all_results).await?;

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
    #[instrument(skip(self, graph, previous_results))]
    async fn execute_wave(
        &self,
        graph: &WorkflowGraph,
        wave: &ExecutionWave,
        previous_results: &HashMap<String, TaskResult>,
    ) -> AgentNetworkResult<Vec<TaskResult>> {
        debug!("Executing wave {}: {} parallel tasks", wave.wave_index, wave.task_indices.len());

        let mut handles: Vec<JoinHandle<AgentNetworkResult<TaskResult>>> = vec![];

        // Spawn all tasks in the wave
        for task_idx in &wave.task_indices {
            let task = graph[*task_idx].clone();
            let agent_pool = Arc::clone(&self.agent_pool);
            let status_stream = Arc::clone(&self.status_stream);
            let coordination = Arc::clone(&self.coordination);
            let file_locks = Arc::clone(&self.file_locks);
            let timeout = self.config.task_timeout;
            let max_retries = self.config.max_retries;
            let wave_index = wave.wave_index;

            let handle = tokio::spawn(async move {
                execute_task_with_retry(
                    task.clone(),
                    agent_pool,
                    status_stream,
                    coordination,
                    file_locks,
                    timeout,
                    max_retries,
                    wave_index,
                )
                .await
            });

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
                    });
                }
                Err(e) => {
                    error!("Join error: {}", e);
                    wave_results.push(TaskResult {
                        task_id: "unknown".to_string(),
                        success: false,
                        output: None,
                        error: Some(format!("Join error: {}", e)),
                    });
                }
            }
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
                    processed.insert(*node_idx);

                    // Respect max concurrent limit per wave
                    if wave_tasks.len() >= self.config.max_concurrent_tasks {
                        break;
                    }
                }
            }

            if wave_tasks.is_empty() {
                break;
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

    /// Get executor configuration
    pub fn config(&self) -> &ExecutorConfig {
        &self.config
    }
}

/// Execute a single task with retry logic
async fn execute_task_with_retry(
    task: TaskNode,
    agent_pool: Arc<AgentPool>,
    status_stream: Arc<StatusStream>,
    coordination: Arc<CoordinationManager>,
    file_locks: Arc<FileLockManager>,
    timeout: Duration,
    max_retries: usize,
    wave_index: usize,
) -> AgentNetworkResult<TaskResult> {
    let task_id = task.task_id.clone();
    let agent_id = task.agent_id.clone();

    // Register task
    coordination.register_task(task_id.clone(), agent_id.clone()).await?;

    // Emit task started event
    status_stream.emit_task_started(
        task_id.clone(),
        agent_id.clone(),
        format!("Task started in wave {}", wave_index),
    );

    let mut retries = 0;
    let mut last_error = None;

    loop {
        // Execute task with timeout
        let result = tokio::time::timeout(timeout, execute_single_task(
            task.clone(),
            Arc::clone(&agent_pool),
            Arc::clone(&file_locks),
        ))
        .await;

        match result {
            Ok(Ok(task_result)) => {
                // Success
                coordination
                    .update_task_status(&task_id, crate::coordination::TaskStatus::Completed)
                    .await?;
                status_stream.emit_task_completed(
                    task_id.clone(),
                    agent_id.clone(),
                    "Task completed successfully".to_string(),
                );
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

    status_stream.emit_task_failed(
        task_id.clone(),
        agent_id.clone(),
        format!("Task failed after {} retries: {}", retries, error_msg),
    );

    Ok(TaskResult {
        task_id,
        success: false,
        output: None,
        error: Some(error_msg),
    })
}

/// Execute a single task
async fn execute_single_task(
    task: TaskNode,
    agent_pool: Arc<AgentPool>,
    file_locks: Arc<FileLockManager>,
) -> AgentNetworkResult<TaskResult> {
    // Get agent
    let agent = agent_pool
        .get_agent(&task.agent_id)
        .ok_or_else(|| AgentNetworkError::Agent(format!("Agent not found: {}", task.agent_id)))?;

    // Create agent context
    let context = AgentContext {
        task_id: task.task_id.clone(),
        description: task.description.clone(),
        rag_context: None,
        history_context: None,
        tool_results: vec![],
        metadata: HashMap::default(),
        conversation_history: vec![],
    };

    // Execute agent
    match agent.execute(context).await {
        Ok(result) => {
            Ok(TaskResult {
                task_id: task.task_id.clone(),
                success: true,
                output: Some(result.output),
                error: None,
            })
        }
        Err(e) => {
            Err(e)
        }
    }
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
