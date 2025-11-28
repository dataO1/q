use axum::{Json, extract::State};
use crate::{
    types::{CapabilitiesResponse, AgentCapability},
    server::AppState,
};

/// List system capabilities and available agent types
pub async fn list_capabilities(
    State(state): State<AppState>,
) -> Json<CapabilitiesResponse> {
    // Use configured agents from system configuration
    let agents: Vec<AgentCapability> = state.config.agent_network.agents
        .iter()
        .map(|agent_config| {
            // Generate description from system prompt or use default
            let description = if agent_config.system_prompt.len() > 100 {
                format!("Agent {} - {}", agent_config.id, 
                       &agent_config.system_prompt[..100].trim())
            } else {
                format!("Agent {} - {}", agent_config.id, agent_config.system_prompt.trim())
            };
            
            // Combine available and required tools
            let mut tools = agent_config.available_tools.clone();
            tools.extend(agent_config.required_tools.clone());
            tools.sort();
            tools.dedup();
            
            AgentCapability {
                agent_type: agent_config.agent_type,
                description,
                tools,
            }
        })
        .collect();

    let features = vec![
        "real_time_streaming".to_string(),
        "multi_agent_orchestration".to_string(),
        "dag_workflow_execution".to_string(),
        "human_in_the_loop".to_string(),
        "smart_rag_integration".to_string(),
        "context_aware_execution".to_string(),
        "tool_integration".to_string(),
        "conflict_resolution".to_string(),
    ];

    Json(CapabilitiesResponse {
        agents,
        features,
        version: "0.1.0".to_string(),
    })
}
