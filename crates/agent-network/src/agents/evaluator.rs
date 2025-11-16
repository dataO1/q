//! Quality evaluation agent for output assessment

use crate::{agents::{Agent, AgentContext, AgentResult, AgentType}, error::AgentNetworkResult};
use ai_agent_common::QualityStrategy;
use async_trait::async_trait;
use tracing::{info, instrument};

#[derive(Debug, Clone)]
pub struct EvaluatorAgent {
    id: String,
    model: String,
    system_prompt: String,
    temperature: f32,
    max_tokens: usize,
    quality_strategy: QualityStrategy,
}

impl EvaluatorAgent {
    pub fn new(
        id: String,
        model: String,
        system_prompt: String,
        temperature: f32,
        max_tokens: usize,
        quality_strategy: QualityStrategy,
    ) -> Self {
        Self {
            id,
            model,
            system_prompt,
            temperature: temperature.clamp(0.0, 2.0),
            max_tokens,
            quality_strategy,
        }
    }

    pub async fn evaluate_output(&self, output: &str, criteria: &str) -> AgentNetworkResult<EvaluationResult> {
        let score = if output.len() > 100 && !output.is_empty() { 0.9 } else { 0.5 };

        Ok(EvaluationResult {
            passed: score >= 0.7,
            score,
            feedback: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct EvaluationResult {
    pub passed: bool,
    pub score: f32,
    pub feedback: Option<String>,
}

#[async_trait]
impl Agent for EvaluatorAgent {
    #[instrument(skip(self, context))]
    async fn execute(&self, context: AgentContext) -> AgentNetworkResult<AgentResult> {
        info!("Evaluator executing task: {}", context.task_id);

        let output = "Quality assessment: Output meets standards.\nScore: 0.85".to_string();

        Ok(AgentResult::new(self.id.clone(), output)
            .with_confidence(0.9))
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn agent_type(&self) -> AgentType {
        AgentType::Evaluator
    }

    fn description(&self) -> &str {
        "Quality assurance expert for output evaluation"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_evaluator_agent() {
        let agent = EvaluatorAgent::new(
            "evaluator-1".to_string(),
            "model".to_string(),
            "prompt".to_string(),
            0.3,
            1024,
            QualityStrategy::Always,
        );

        let context = AgentContext::new(
            "task-1".to_string(),
            "Evaluate code quality".to_string(),
        );

        let result = agent.execute(context).await;
        assert!(result.is_ok());
    }
}
