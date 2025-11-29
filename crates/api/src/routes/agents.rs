use axum::{Json, extract::State};
use serde_json::json;
use tracing::{info, instrument};
use crate::{
    types::{CapabilitiesResponse, AgentCapability},
    server::AppState,
};

/// Get system capabilities and agent discovery
/// 
/// Returns information about available agents, supported features, and API capabilities.
/// Use this endpoint for agent discovery and capability negotiation.
/// 
/// ## Agent Types
/// 
/// Each agent type specializes in different kinds of tasks:
/// - **Coding**: Code analysis, implementation, debugging, refactoring
/// - **Planning**: Task decomposition, workflow planning, architecture design
/// - **Writing**: Documentation, explanations, summaries, communication
/// - **Evaluator**: Code review, quality assessment, testing strategies
/// 
/// ## Features
/// 
/// The API supports various advanced features:
/// - `real_time_streaming`: WebSocket status updates during execution
/// - `multi_agent_orchestration`: Automatic task routing to appropriate agents
/// - `dag_workflow_execution`: Complex workflow with dependency management
/// - `human_in_the_loop`: Human approval for sensitive operations
/// - `smart_rag_integration`: Context-aware information retrieval
/// - `context_aware_execution`: Project-specific tool and approach selection
/// 
/// ## Usage
/// 
/// Use this information to:
/// - Discover available agent capabilities
/// - Determine which agents can handle specific tasks
/// - Check API feature support before making requests
/// - Display agent options in client interfaces
#[utoipa::path(
    get,
    path = "/capabilities",
    tag = "discovery",
    responses(
        (status = 200, description = "System capabilities", body = CapabilitiesResponse,
         example = json!({
             "agents": [
                 {
                     "agent_type": "Coding",
                     "description": "Analyzes code, implements features, fixes bugs",
                     "tools": ["file_reader", "file_writer", "lsp_client"]
                 },
                 {
                     "agent_type": "Planning", 
                     "description": "Decomposes tasks and plans workflows",
                     "tools": ["task_planner", "dependency_analyzer"]
                 }
             ],
             "features": ["real_time_streaming", "multi_agent_orchestration"],
             "version": "1.0.0"
         }))
    )
)]
#[instrument(skip(state))]
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

    info!(
        agent_count = %agents.len(),
        feature_count = %features.len(),
        version = "0.1.0",
        "Returning system capabilities"
    );

    Json(CapabilitiesResponse {
        agents,
        features,
        version: "0.1.0".to_string(),
    })
}
