//! Workflow analysis and complexity metrics
//!
//! Provides analysis of workflow DAGs including complexity metrics,
//! critical path identification, and execution estimates.

use crate::error::AgentNetworkResult;
use crate::workflow::{WorkflowGraph, TaskNode};
use petgraph::algo::{toposort, dijkstra};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::{HashMap, VecDeque};
use tracing::{debug, info};

/// Workflow analysis report
#[derive(Debug, Clone)]
pub struct WorkflowAnalysis {
    pub node_count: usize,
    pub edge_count: usize,
    pub is_acyclic: bool,
    pub complexity: WorkflowComplexity,
    pub critical_path: Vec<String>,
    pub critical_path_length: usize,
    pub estimated_waves: usize,
    pub parallelism_factor: f32,
    pub agent_distribution: HashMap<String, usize>,
}

/// Workflow complexity metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkflowComplexity {
    Trivial,
    Simple,
    Moderate,
    Complex,
    VeryComplex,
}

impl std::fmt::Display for WorkflowComplexity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trivial => write!(f, "Trivial"),
            Self::Simple => write!(f, "Simple"),
            Self::Moderate => write!(f, "Moderate"),
            Self::Complex => write!(f, "Complex"),
            Self::VeryComplex => write!(f, "VeryComplex"),
        }
    }
}

/// Workflow analyzer
pub struct WorkflowAnalyzer;

impl WorkflowAnalyzer {
    /// Analyze workflow graph
    pub fn analyze(graph: &WorkflowGraph) -> AgentNetworkResult<WorkflowAnalysis> {
        let node_count = graph.node_count();
        let edge_count = graph.edge_count();

        debug!("Analyzing workflow: {} nodes, {} edges", node_count, edge_count);

        // Check if acyclic
        let is_acyclic = toposort(graph, None).is_ok();

        if !is_acyclic {
            return Err(crate::error::AgentNetworkError::dag_construction(
                "Workflow contains cycles",
            ));
        }

        // Get topological ordering
        let sorted = toposort(graph, None).map_err(|_| {
            crate::error::AgentNetworkError::dag_construction("Failed to sort workflow")
        })?;

        // Find critical path
        let critical_path = Self::find_critical_path(graph, &sorted);
        let critical_path_length = critical_path.len();

        // Estimate execution waves
        let estimated_waves = Self::estimate_waves(graph, &sorted);

        // Calculate parallelism factor
        let parallelism_factor = Self::calculate_parallelism_factor(node_count, estimated_waves);

        // Determine complexity
        let complexity = Self::determine_complexity(node_count, edge_count, estimated_waves);

        // Count agents by type
        let agent_distribution = Self::analyze_agent_distribution(graph);

        info!("Workflow analysis complete: {} complexity, {} waves, {:.2}x parallelism",
              complexity, estimated_waves, parallelism_factor);

        Ok(WorkflowAnalysis {
            node_count,
            edge_count,
            is_acyclic,
            complexity,
            critical_path,
            critical_path_length,
            estimated_waves,
            parallelism_factor,
            agent_distribution,
        })
    }

    /// Validate workflow for potential issues
    pub fn validate(graph: &WorkflowGraph) -> AgentNetworkResult<Vec<String>> {
        let mut issues = vec![];

        // Check for cycles
        if toposort(graph, None).is_err() {
            issues.push("Workflow contains cycles".to_string());
        }

        // Check for isolated nodes
        for node_idx in graph.node_indices() {
            let in_degree = graph.edges_directed(node_idx, Direction::Incoming).count();
            let out_degree = graph.edges_directed(node_idx, Direction::Outgoing).count();

            if in_degree == 0 && out_degree == 0 && graph.node_count() > 1 {
                let task = &graph[node_idx];
                issues.push(format!("Isolated node: {}", task.task_id));
            }
        }

        // Check for unreachable nodes
        if let Ok(sorted) = toposort(graph, None) {
            if sorted.len() != graph.node_count() {
                issues.push("Some nodes are unreachable".to_string());
            }
        }

        Ok(issues)
    }

    /// Find critical path in DAG
    fn find_critical_path(graph: &WorkflowGraph, sorted_nodes: &[NodeIndex]) -> Vec<String> {
        if sorted_nodes.is_empty() {
            return vec![];
        }

        // Find nodes with no incoming edges (start nodes)
        let start_nodes: Vec<_> = sorted_nodes
            .iter()
            .filter(|node_idx| {
                graph
                    .edges_directed(**node_idx, Direction::Incoming)
                    .next()
                    .is_none()
            })
            .collect();

        if start_nodes.is_empty() {
            return vec![];
        }

        // Simple critical path: longest path from start to end
        let mut longest_path = vec![];

        for start_node in start_nodes {
            let path = Self::find_longest_path(graph, *start_node, sorted_nodes);
            if path.len() > longest_path.len() {
                longest_path = path;
            }
        }

        longest_path
            .iter()
            .map(|node_idx| graph[*node_idx].task_id.clone())
            .collect()
    }

    /// Find longest path from a node
    fn find_longest_path(
        graph: &WorkflowGraph,
        start: NodeIndex,
        _sorted_nodes: &[NodeIndex],
    ) -> Vec<NodeIndex> {
        let mut longest = vec![start];
        let mut queue = VecDeque::new();
        queue.push_back(vec![start]);

        while let Some(path) = queue.pop_front() {
            let current = *path.last().unwrap();

            let mut found_successor = false;
            for edge in graph.edges_directed(current, Direction::Outgoing) {
                found_successor = true;
                let target = edge.target();
                let mut new_path = path.clone();
                new_path.push(target);

                if new_path.len() > longest.len() {
                    longest = new_path.clone();
                }

                queue.push_back(new_path);
            }

            if !found_successor && path.len() > longest.len() {
                longest = path;
            }
        }

        longest
    }

    /// Estimate number of execution waves
    fn estimate_waves(graph: &WorkflowGraph, sorted_nodes: &[NodeIndex]) -> usize {
        if sorted_nodes.is_empty() {
            return 0;
        }

        let mut wave_count = 1;
        let mut processed = std::collections::HashSet::new();

        while processed.len() < sorted_nodes.len() {
            let mut wave_size = 0;

            for node_idx in sorted_nodes {
                if processed.contains(node_idx) {
                    continue;
                }

                // Check dependencies
                let mut all_deps_met = true;
                for edge in graph.edges_directed(*node_idx, Direction::Incoming) {
                    if !processed.contains(&edge.source()) {
                        all_deps_met = false;
                        break;
                    }
                }

                if all_deps_met {
                    processed.insert(*node_idx);
                    wave_size += 1;
                }
            }

            if wave_size == 0 {
                break;
            }

            wave_count += 1;
        }

        wave_count
    }

    /// Calculate parallelism factor (ideal speedup potential)
    fn calculate_parallelism_factor(node_count: usize, wave_count: usize) -> f32 {
        if wave_count == 0 {
            1.0
        } else {
            node_count as f32 / wave_count as f32
        }
    }

    /// Determine overall complexity
    fn determine_complexity(node_count: usize, edge_count: usize, waves: usize) -> WorkflowComplexity {
        let density = if node_count > 0 {
            edge_count as f32 / (node_count as f32 * (node_count - 1) as f32)
        } else {
            0.0
        };

        match (node_count, waves, density) {
            (0..=1, _, _) => WorkflowComplexity::Trivial,
            (2..=5, w, _) if w <= 2 => WorkflowComplexity::Simple,
            (2..=10, w, d) if w <= 3 && d < 0.3 => WorkflowComplexity::Simple,
            (6..=20, w, d) if w <= 5 && d < 0.5 => WorkflowComplexity::Moderate,
            (11..=50, _, _) => WorkflowComplexity::Complex,
            _ => WorkflowComplexity::VeryComplex,
        }
    }

    /// Analyze agent distribution
    fn analyze_agent_distribution(graph: &WorkflowGraph) -> HashMap<String, usize> {
        let mut distribution = HashMap::new();

        for node_idx in graph.node_indices() {
            let task = &graph[node_idx];
            *distribution.entry(task.agent_id.clone()).or_insert(0) += 1;
        }

        distribution
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_ordering() {
        assert!(WorkflowComplexity::Trivial < WorkflowComplexity::Simple);
        assert!(WorkflowComplexity::Simple < WorkflowComplexity::Moderate);
        assert!(WorkflowComplexity::Complex < WorkflowComplexity::VeryComplex);
    }

    #[test]
    fn test_parallelism_factor() {
        assert_eq!(WorkflowAnalyzer::calculate_parallelism_factor(10, 1), 10.0);
        assert_eq!(WorkflowAnalyzer::calculate_parallelism_factor(10, 5), 2.0);
        assert_eq!(WorkflowAnalyzer::calculate_parallelism_factor(0, 0), 1.0);
    }
}
