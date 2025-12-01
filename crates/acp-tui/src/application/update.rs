//! Application update logic (Update in Elm architecture)

use anyhow::Result;
use tracing::{debug, info};
use chrono::Utc;

use crate::{
    application::AppModel,
    message::{AppMsg, StatusSeverity},
    client::types::{StatusEvent, EventType, EventSource},
};

/// Update function - handles all application messages and updates the model
/// This is the core Update function from Elm architecture
pub fn update_app(model: &mut AppModel, msg: AppMsg) -> Result<Vec<AppMsg>> {
    let mut effects = Vec::new(); // Side effects to be handled
    
    match msg {
        // ============== System Events ==============
        AppMsg::Quit => {
            info!("Application quit requested");
            // No model changes needed, handled by main loop
        }
        
        AppMsg::TerminalResized(width, height) => {
            debug!("Terminal resized to {}x{}", width, height);
            // Terminal resize affects all components
            model.component_dirty_flags.mark_all_dirty();
        }
        
        AppMsg::Tick => {
            model.tick_animation();
            // Animation tick only affects timeline
            model.component_dirty_flags.timeline = true;
        }
        
        // ============== Connection Events ==============
        AppMsg::StartConnection => {
            model.set_connection_state(crate::components::realm::status_line::ConnectionState::Connecting);
            model.set_status_message(StatusSeverity::Info, "Connecting to ACP server...".to_string());
            // Connection events affect status line
            model.component_dirty_flags.status_line = true;
        }
        
        AppMsg::SubscriptionCreated(subscription_id) => {
            info!("Subscription created: {}", subscription_id);
            model.set_status_message(StatusSeverity::Info, 
                format!("Connected (subscription: {})", subscription_id));
            model.component_dirty_flags.status_line = true;
        }
        
        AppMsg::SubscriptionResumed(subscription_id) => {
            info!("Subscription resumed: {}", subscription_id);
            model.set_status_message(StatusSeverity::Info, 
                format!("Reconnected (subscription: {})", subscription_id));
            model.component_dirty_flags.status_line = true;
        }
        
        AppMsg::WebSocketConnected => {
            model.set_connection_state(crate::components::realm::status_line::ConnectionState::Connected);
            model.set_status_message(StatusSeverity::Info, "WebSocket connected".to_string());
            model.component_dirty_flags.status_line = true;
        }
        
        AppMsg::WebSocketDisconnected => {
            model.set_connection_state(crate::components::realm::status_line::ConnectionState::Disconnected);
            model.set_status_message(StatusSeverity::Warning, "WebSocket disconnected".to_string());
            model.component_dirty_flags.status_line = true;
        }
        
        AppMsg::ConnectionFailed(error) => {
            model.set_connection_state(crate::components::realm::status_line::ConnectionState::Failed { 
                error: error.clone() 
            });
            model.set_status_message(StatusSeverity::Error, 
                format!("Connection failed: {}", error));
            model.component_dirty_flags.status_line = true;
        }
        
        // ============== Query Events ==============
        AppMsg::QueryInputChanged(text) => {
            model.set_query(text);
            // Query input changes affect query input component
            model.component_dirty_flags.query_input = true;
        }
        
        AppMsg::QuerySubmitted => {
            if !model.query_text.trim().is_empty() {
                info!("Query submitted: {}", model.query_text);
                model.last_execution_time = Some(Utc::now());
                // Clear query after submission
                let query = model.query_text.clone();
                model.clear_query();
                // Trigger query execution effect
                effects.push(AppMsg::QueryExecutionStarted(query));
            }
        }
        
        AppMsg::QueryExecutionStarted(query) => {
            model.set_status_message(StatusSeverity::Info, 
                format!("Executing query: {}", query));
        }
        
        AppMsg::QueryExecutionCompleted(result) => {
            model.set_status_message(StatusSeverity::Info, 
                format!("Query completed: {}", result));
        }
        
        AppMsg::QueryExecutionFailed(error) => {
            model.set_status_message(StatusSeverity::Error, 
                format!("Query failed: {}", error));
        }
        
        // ============== Timeline Events ==============
        AppMsg::StatusEventReceived(event) => {
            handle_status_event(model, event)?;
            // Status events affect timeline and status line
            model.component_dirty_flags.timeline = true;
            model.component_dirty_flags.status_line = true;
        }
        
        AppMsg::TimelineScrollUp => {
            model.scroll_timeline_up();
        }
        
        AppMsg::TimelineScrollDown => {
            model.scroll_timeline_down();
        }
        
        AppMsg::TimelineNodeToggle(node_id) => {
            model.timeline_tree.toggle_expanded(&node_id);
        }
        
        AppMsg::TimelineClear => {
            model.timeline_tree.clear();
            model.timeline_scroll = 0;
            model.set_status_message(StatusSeverity::Info, "Timeline cleared".to_string());
        }
        
        // ============== HITL Events ==============
        AppMsg::HitlRequestReceived(request) => {
            model.add_hitl_request(request);
            model.switch_to_hitl_layout();
        }
        
        AppMsg::HitlReviewOpen(request_id) => {
            if let Some(request) = model.remove_hitl_request(&request_id) {
                model.current_hitl_request = Some(request);
                model.focused_component = crate::message::ComponentId::HitlReview;
            }
        }
        
        AppMsg::HitlReviewClose => {
            model.current_hitl_request = None;
            model.switch_to_normal_layout();
        }
        
        AppMsg::HitlDecisionMade(request_id, decision) => {
            info!("HITL decision made for {}: {:?}", request_id, decision);
            model.current_hitl_request = None;
            if model.hitl_requests.is_empty() {
                model.switch_to_normal_layout();
            }
        }
        
        AppMsg::HitlDecisionSent(request_id) => {
            model.set_status_message(StatusSeverity::Info, 
                format!("HITL decision sent for {}", request_id));
        }
        
        AppMsg::HitlDecisionFailed(request_id, error) => {
            model.set_status_message(StatusSeverity::Error, 
                format!("HITL decision failed for {}: {}", request_id, error));
        }
        
        // ============== UI Navigation Events ==============
        AppMsg::FocusNext => {
            model.focus_next_component();
        }
        
        AppMsg::FocusPrevious => {
            model.focus_previous_component();
        }
        
        AppMsg::FocusComponent(component_id) => {
            model.focused_component = component_id;
        }
        
        AppMsg::HelpToggle => {
            model.toggle_help();
        }
        
        // ============== Layout Events ==============
        AppMsg::LayoutNormal => {
            model.switch_to_normal_layout();
        }
        
        AppMsg::LayoutHitlReview => {
            model.switch_to_hitl_layout();
        }
        
        // ============== Error Events ==============
        AppMsg::ErrorOccurred(error) => {
            model.set_status_message(StatusSeverity::Error, error);
        }
        
        AppMsg::StatusMessage(severity, message) => {
            model.set_status_message(severity, message);
        }
    }
    
    Ok(effects)
}

/// Handle status events from WebSocket and update timeline
fn handle_status_event(model: &mut AppModel, event: StatusEvent) -> Result<()> {
    debug!("Processing status event: {:?}", event.event);
    
    match &event.event {
        EventType::ExecutionStarted { query } => {
            let root = model.timeline_tree.add_root(
                event.execution_id.clone(),
                format!("Query: {}", query)
            );
            root.start();
        }
        
        EventType::AgentStarted { context_size } => {
            if let EventSource::Agent { agent_id, agent_type, task_id: _ } = &event.source {
                model.timeline_tree.add_child(
                    event.execution_id.clone(),
                    agent_id.clone(),
                    format!("{:?} Agent (ctx: {})", agent_type, context_size)
                );
                if let Some(node) = model.timeline_tree.find_node_mut(agent_id) {
                    node.start();
                }
            }
        }
        
        EventType::AgentCompleted { result: _ } => {
            if let EventSource::Agent { agent_id, .. } = &event.source {
                if let Some(node) = model.timeline_tree.find_node_mut(agent_id) {
                    node.complete();
                }
            }
        }
        
        EventType::AgentFailed { error } => {
            if let EventSource::Agent { agent_id, .. } = &event.source {
                if let Some(node) = model.timeline_tree.find_node_mut(agent_id) {
                    node.fail(Some(error.clone()));
                }
            }
        }
        
        EventType::ToolStarted { args } => {
            if let EventSource::Tool { agent_id, tool_name } = &event.source {
                let tool_id = format!("{}:{}", agent_id, tool_name);
                model.timeline_tree.add_child(
                    agent_id.clone(),
                    tool_id.clone(),
                    format!("Tool: {} ({:?})", tool_name, args)
                );
                if let Some(node) = model.timeline_tree.find_node_mut(&tool_id) {
                    node.start();
                }
            }
        }
        
        EventType::ToolCompleted { result: _ } => {
            if let EventSource::Tool { agent_id, tool_name } = &event.source {
                let tool_id = format!("{}:{}", agent_id, tool_name);
                if let Some(node) = model.timeline_tree.find_node_mut(&tool_id) {
                    node.complete();
                }
            }
        }
        
        EventType::ToolFailed { error } => {
            if let EventSource::Tool { agent_id, tool_name } = &event.source {
                let tool_id = format!("{}:{}", agent_id, tool_name);
                if let Some(node) = model.timeline_tree.find_node_mut(&tool_id) {
                    node.fail(Some(error.clone()));
                }
            }
        }
        
        EventType::ExecutionCompleted { result: _ } => {
            if let Some(root) = model.timeline_tree.find_node_mut(&event.execution_id) {
                root.complete();
            }
        }
        
        EventType::ExecutionFailed { error } => {
            if let Some(root) = model.timeline_tree.find_node_mut(&event.execution_id) {
                root.fail(Some(error.clone()));
            }
        }
        
        _ => {
            debug!("Unhandled event type: {:?}", event.event);
        }
    }
    
    Ok(())
}