//! Complexity analysis for tasks in the workflow graph


/// Analyzes task complexity to decide decomposition or direct execution
pub struct ComplexityAnalyzer;

impl ComplexityAnalyzer {
    pub fn new() -> Self {
        Self {}
    }

    /// Analyze the input task description and return complexity estimate
    ///
    /// Returns true if task is complex enough to require decomposition
    pub fn is_complex_task(&self, task: &str) -> bool {
        // Placeholder heuristic:
        task.len() > 100 || task.contains("refactor") || task.contains("architect")
    }

    /// Optionally compute complexity score (e.g. 0-1)
    pub fn complexity_score(&self, task: &str) -> f32 {
        let length_score = (task.len() as f32) / 500.0;
        let keyword_score = if task.contains("refactor") || task.contains("architect") {
            0.5
        } else {
            0.0
        };
        (length_score + keyword_score).min(1.0)
    }
}
