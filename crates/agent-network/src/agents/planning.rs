//! Planning agent for task decomposition
//!
//! The planning agent breaks down complex queries into executable subtasks
//! and generates execution strategies.

use crate::{agents::{Agent, AgentContext, AgentResult, AgentType}, error::AgentNetworkResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

/// Planning agent for strategic decomposition
pub struct PlanningAgent {
    id: String,
    model: String,
    system_prompt: String,
    temperature: f32,
    max_tokens: usize,
}

impl PlanningAgent {
    /// Create a new planning agent
    pub fn new(
        id: String,
        model: String,
        system_prompt: String,
        temperature: f32,
        max_tokens: usize,
    ) -> Self {
        Self {
            id,
            model,
            system_prompt,
            temperature: temperature.clamp(0.0, 2.0),
            max_tokens,
        }
    }

    /// Parse plan output into structured tasks
    fn parse_plan(&self, output: &str) -> Vec<PlanStep> {
        let mut steps = vec![];
        let mut current_step = 1;

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Match patterns like "1.", "Step 1:", "- ", etc.
            if trimmed.starts_with(char::is_numeric) && (trimmed.contains('.') || trimmed.contains(':')) {
                let content = if let Some(idx) = trimmed.find('.') {
                    &trimmed[idx + 1..]
                } else if let Some(idx) = trimmed.find(':') {
                    &trimmed[idx + 1..]
                } else {
                    trimmed
                };

                steps.push(PlanStep {
                    order: current_step,
                    description: content.trim().to_string(),
                    dependencies: vec![],
                });

                current_step += 1;
            } else if trimmed.starts_with('-') || trimmed.starts_with('*') {
                let content = trimmed.trim_start_matches(|c: char| c == '-' || c == '*' || c.is_whitespace());
                steps.push(PlanStep {
                    order: current_step,
                    description: content.to_string(),
                    dependencies: vec![],
                });

                current_step += 1;
            }
        }

        steps
    }
}

/// Represents a single planning step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub order: usize,
    pub description: String,
    pub dependencies: Vec<usize>,
}

#[async_trait]
impl Agent for PlanningAgent {
    #[instrument(skip(self, context), fields(agent_id = %self.id, task_id = %context.task_id))]
    async fn execute(&self, context: AgentContext) -> AgentNetworkResult<AgentResult> {
        info!("Planning agent executing task: {}", context.task_id);

        let prompt = format!(
            "Please break down this task into clear, sequential steps:\n\n{}\n\n\
             Format each step as a numbered list with:\n\
             1. Clear action\n\
             2. Expected outcome\n\
             3. Any dependencies",
            context.description
        );

        debug!("Planning prompt: {}", prompt);

        // TODO: Week 3 - Integrate with Rig framework
        // - Call LLM with planning prompt
        // - Parse structured response
        // - Extract task dependencies

        let output = format!(
            "Plan for: {}\n\n\
             1. Analyze requirements\n\
             2. Design solution\n\
             3. Implement\n\
             4. Test\n\
             5. Review and refactor\n\n\
             Each step has clear dependencies on the previous step.",
            context.description
        );

        let plan_steps = self.parse_plan(&output);
        let confidence = if plan_steps.len() >= 3 { 0.85 } else { 0.6 };

        Ok(AgentResult::new(self.id.clone(), output)
            .with_confidence(confidence)
            .with_reasoning(format!("Generated {} plan steps", plan_steps.len())))
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn agent_type(&self) -> AgentType {
        AgentType::Planning
    }

    fn description(&self) -> &str {
        "Strategic planning agent for task decomposition and analysis"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planning_agent_creation() {
        let agent = PlanningAgent::new(
            "planning-1".to_string(),
            "qwen2.5:14b".to_string(),
            "You are a strategic planner".to_string(),
            0.8,
            2048,
        );

        assert_eq!(agent.id(), "planning-1");
        assert_eq!(agent.agent_type(), AgentType::Planning);
    }

    #[test]
    fn test_parse_numbered_list() {
        let agent = PlanningAgent::new(
            "test".to_string(),
            "model".to_string(),
            "prompt".to_string(),
            0.8,
            1024,
        );

        let output = "1. First step\n2. Second step\n3. Third step";
        let steps = agent.parse_plan(output);

        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].order, 1);
        assert_eq!(steps[0].description, "First step");
        assert_eq!(steps[1].order, 2);
        assert_eq!(steps[1].description, "Second step");
    }

    #[test]
    fn test_parse_bullet_list() {
        let agent = PlanningAgent::new(
            "test".to_string(),
            "model".to_string(),
            "prompt".to_string(),
            0.8,
            1024,
        );

        let output = "- First item\n- Second item\n- Third item";
        let steps = agent.parse_plan(output);

        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].order, 1);
        assert_eq!(steps[0].description, "First item");
    }

    #[test]
    fn test_parse_mixed_format() {
        let agent = PlanningAgent::new(
            "test".to_string(),
            "model".to_string(),
            "prompt".to_string(),
            0.8,
            1024,
        );

        let output = "Step 1: Analysis\nStep 2: Design\n3. Implementation";
        let steps = agent.parse_plan(output);

        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].description, "Analysis");
        assert_eq!(steps[2].description, "Implementation");
    }

    #[test]
    fn test_parse_empty_input() {
        let agent = PlanningAgent::new(
            "test".to_string(),
            "model".to_string(),
            "prompt".to_string(),
            0.8,
            1024,
        );

        let output = "";
        let steps = agent.parse_plan(output);

        assert_eq!(steps.len(), 0);
    }

    #[tokio::test]
    async fn test_execute_placeholder() {
        let agent = PlanningAgent::new(
            "planning-1".to_string(),
            "model".to_string(),
            "You are helpful".to_string(),
            0.8,
            2048,
        );

        let context = AgentContext::new(
            "task-1".to_string(),
            "Build a web server".to_string(),
        );

        let result = agent.execute(context).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.agent_id, "planning-1");
        assert!(!result.output.is_empty());
        assert!(result.confidence > 0.5);
    }
}
