//! Coding agent implementation using Rig LLM framework
//!
//! The coding agent specializes in generating, reviewing, and refactoring code.
//! It integrates with Rig for LLM calls and supports local Ollama models.

use crate::{ agents::{Agent, AgentContext, AgentResult, AgentType}, error::AgentNetworkResult};
use async_trait::async_trait;
use tracing::{debug, info, instrument};

/// Coding agent for code generation and review
pub struct CodingAgent {
    /// Unique agent identifier
    id: String,

    /// LLM model name (e.g., "qwen2.5-coder:32b")
    model: String,

    /// System prompt for the agent
    system_prompt: String,

    /// Temperature for responses (0.0-2.0)
    temperature: f32,

    /// Maximum tokens for output
    max_tokens: usize,
}

impl CodingAgent {
    /// Create a new coding agent
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

    /// Build the prompt for the LLM
    fn build_prompt(&self, context: &AgentContext) -> String {
        let mut prompt = format!("Task ID: {}\n\n", context.task_id);
        prompt.push_str(&format!("Task: {}\n\n", context.description));

        if !context.tool_results.is_empty() {
            prompt.push_str("## Available Information\n");
            for tool_result in &context.tool_results {
                if tool_result.success {
                    prompt.push_str(&format!("From {}: {}\n", tool_result.tool_name, tool_result.output));
                }
            }
            prompt.push_str("\n");
        }

        if let Some(rag_context) = &context.rag_context {
            prompt.push_str("## Relevant Code Examples\n");
            prompt.push_str(rag_context);
            prompt.push_str("\n\n");
        }

        prompt.push_str("Please provide your response in the following format:\n");
        prompt.push_str("1. Analysis\n");
        prompt.push_str("2. Implementation\n");
        prompt.push_str("3. Explanation\n");

        prompt
    }

    /// Estimate confidence based on prompt characteristics
    fn estimate_confidence(&self, output: &str) -> f32 {
        let has_code = output.contains("```") || output.contains("fn ");
        let is_complete = output.len() > 200;
        let has_explanation = output.to_lowercase().contains("explanation")
            || output.to_lowercase().contains("why")
            || output.to_lowercase().contains("because");

        let mut confidence: f32 = 0.6;
        if has_code {
            confidence += 0.2;
        }
        if is_complete {
            confidence += 0.1;
        }
        if has_explanation {
            confidence += 0.1;
        }

        confidence.min(0.99)
    }
}

#[async_trait]
impl Agent for CodingAgent {
    #[instrument(skip(self, context), fields(agent_id = %self.id, task_id = %context.task_id))]
    async fn execute(&self, context: AgentContext) -> AgentNetworkResult<AgentResult> {
        info!("Coding agent executing task: {}", context.task_id);

        let prompt = self.build_prompt(&context);
        debug!("Prompt length: {} characters", prompt.len());

        // TODO: Week 3 - Integrate with Rig framework
        // - Connect to local Ollama instance
        // - Send prompt with system message
        // - Stream response
        // - Parse output

        // Placeholder implementation
        let output = format!(
            "```rust\n// Implementation for: {}\nfn solution() {{\n    // TODO: implement\n}}\n```\n\n\
             Analysis: Task requires code generation.\n\
             Implementation: Generated placeholder code structure.\n\
             Explanation: Code is structured to follow Rust best practices.",
            context.description
        );

        let confidence = self.estimate_confidence(&output);

        Ok(AgentResult::new(self.id.clone(), output.clone())
            .with_confidence(confidence)
            .with_tokens(estimate_tokens(&prompt, &output)))
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn agent_type(&self) -> AgentType {
        AgentType::Coding
    }

    fn description(&self) -> &str {
        "Expert Rust coding agent for implementation and code review"
    }
}

/// Estimate tokens for given text
fn estimate_tokens(prompt: &str, output: &str) -> usize {
    // Rough estimate: 1 token ≈ 4 characters
    // Add 10% buffer for tokenization inefficiency
    ((prompt.len() + output.len()) / 4) + ((prompt.len() + output.len()) / 40)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coding_agent_creation() {
        let agent = CodingAgent::new(
            "coding-1".to_string(),
            "qwen2.5-coder:32b".to_string(),
            "You are a Rust expert".to_string(),
            0.7,
            4096,
        );

        assert_eq!(agent.id(), "coding-1");
        assert_eq!(agent.agent_type(), AgentType::Coding);
    }

    #[test]
    fn test_temperature_clamping() {
        let agent = CodingAgent::new(
            "test".to_string(),
            "model".to_string(),
            "prompt".to_string(),
            3.0, // Should clamp to 2.0
            1024,
        );

        assert_eq!(agent.temperature, 2.0);
    }

    #[test]
    fn test_confidence_estimation() {
        let agent = CodingAgent::new(
            "test".to_string(),
            "model".to_string(),
            "prompt".to_string(),
            0.7,
            1024,
        );

        let output_with_code = "```rust\nfn test() {}\n```\nExplanation: This is a test function.";
        let confidence = agent.estimate_confidence(output_with_code);
        assert!(confidence > 0.7);

        let output_minimal = "test";
        let confidence_low = agent.estimate_confidence(output_minimal);
        assert!(confidence_low < 0.7);
    }

    #[tokio::test]
    async fn test_execute_placeholder() {
        let agent = CodingAgent::new(
            "coding-1".to_string(),
            "model".to_string(),
            "You are helpful".to_string(),
            0.7,
            4096,
        );

        let context = AgentContext::new("task-1".to_string(), "Write a binary search function".to_string());

        let result = agent.execute(context).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.agent_id, "coding-1");
        assert!(!result.output.is_empty());
    }
}
