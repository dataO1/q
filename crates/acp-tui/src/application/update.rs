//! Application update logic (Update in Elm architecture)

use anyhow::Result;
use tracing::{debug, info, instrument, warn};
use chrono::Utc;

use crate::{
    application::AppModel,
    message::{AppMsg, StatusSeverity},
    components::realm::status_line::ConnectionState,
    client::types::{StatusEvent, EventType, EventSource},
};

/// Update function - handles all application messages and updates the model
/// This is the core Update function from Elm architecture
#[instrument(level = "debug", skip(model), fields(
    msg_type = %format!("{:?}", msg).split('(').next().unwrap_or("Unknown"),
    query_text_len = model.query_text.len(),
    focused_component = ?model.focused_component,
))]
pub fn update(model: &mut AppModel, msg: AppMsg) -> Result<Vec<AppMsg>> {
    let mut effects = Vec::new(); // Side effects to be handled

    match msg {
        // ============== System Events ==============
        AppMsg::Quit => {
            info!("Application quit requested");
            // No model changes needed, handled by main loop
        }

        AppMsg::TerminalResized(width, height) => {
            debug!(width, height, "Terminal resized");
            // Terminal resize affects all components
        }

        AppMsg::Tick => {
            model.tick_animation();
            // Animation tick only affects timeline
        }

        // ============== Connection Events ==============
        AppMsg::StartConnection => {
            info!("Starting connection to ACP server");
            model.set_connection_state(crate::components::realm::status_line::ConnectionState::Connecting);
            model.set_status_message(StatusSeverity::Info, "Connecting to ACP server...".to_string());
        }

        AppMsg::SubscriptionCreated(subscription_id) => {
            info!(subscription_id = %subscription_id, "Subscription created");
            model.subscription_id = Some(subscription_id.clone());
            model.connection_state = ConnectionState::Connected;
            model.set_status_message(StatusSeverity::Info,
                format!("Connected (subscription: {})", subscription_id));
        }

        AppMsg::SubscriptionResumed(subscription_id) => {
            info!(subscription_id = %subscription_id, "Subscription resumed");
            model.subscription_id = Some(subscription_id.clone());
            model.connection_state = ConnectionState::Connected;
            model.set_status_message(StatusSeverity::Info,
                format!("Reconnected (subscription: {})", subscription_id));
        }

        AppMsg::WebSocketConnected => {
            info!("WebSocket connected");
            model.set_connection_state(crate::components::realm::status_line::ConnectionState::Connected);
            model.set_status_message(StatusSeverity::Info, "WebSocket connected".to_string());
        }

        AppMsg::WebSocketDisconnected => {
            warn!("WebSocket disconnected");
            model.subscription_id = None;
            model.set_connection_state(crate::components::realm::status_line::ConnectionState::Disconnected);
            model.set_status_message(StatusSeverity::Warning, "WebSocket disconnected".to_string());
        }

        AppMsg::ConnectionFailed(error) => {
            warn!(error = %error, "Connection failed");
            model.subscription_id = None;
            model.set_connection_state(crate::components::realm::status_line::ConnectionState::Failed {
                error: error.clone()
            });
            model.set_status_message(StatusSeverity::Error,
                format!("Connection failed: {}", error));
        }

        // ============== Query Events ==============
        AppMsg::QueryInputChanged(text) => {
            debug!(text_len = text.len(), "Query input changed");
            model.set_query(text);
        }

        AppMsg::QuerySubmitted => {
            if !model.query_text.trim().is_empty() {
                let query = model.query_text.clone();
                info!(query = %query, query_len = query.len(), "Query submitted");
                model.last_execution_time = Some(Utc::now());

                // Clear query after submission
                model.clear_query();

                // Trigger query execution effect
                debug!(query = %query, "Creating QueryExecutionStarted effect");
                effects.push(AppMsg::QueryExecutionStarted(query));
            } else {
                warn!("Query submission attempted with empty query text");
            }
        }

        AppMsg::QueryExecutionStarted(query) => {
            info!(query = %query, "Query execution started");
            model.set_status_message(StatusSeverity::Info,
                format!("Executing query: {}", query));
        }

        AppMsg::QueryExecutionCompleted(result) => {
            info!(result = %result, "Query execution completed");
            model.set_status_message(StatusSeverity::Info,
                format!("Query completed: {}", result));
        }

        AppMsg::QueryExecutionFailed(error) => {
            warn!(error = %error, "Query execution failed");
            model.set_status_message(StatusSeverity::Error,
                format!("Query failed: {}", error));
        }

        // ============== Timeline Events ==============
        AppMsg::StatusEventReceived(event) => {
            debug!(event_type = ?event.event, execution_id = %event.execution_id, "Status event received");
            handle_status_event(model, event)?;
        }

        AppMsg::TimelineScrollUp => {
            debug!("Timeline scroll up");
            model.scroll_timeline_up();
        }

        AppMsg::TimelineScrollDown => {
            debug!("Timeline scroll down");
            model.scroll_timeline_down();
        }

        AppMsg::TimelineNodeToggle(node_id) => {
            debug!(node_id = %node_id, "Timeline node toggle");
            model.timeline_tree.toggle_expanded(&node_id);
        }

        AppMsg::TimelineClear => {
            info!("Timeline cleared");
            model.timeline_tree.clear();
            model.timeline_scroll = 0;
            model.set_status_message(StatusSeverity::Info, "Timeline cleared".to_string());
        }

        // ============== HITL Events ==============
        AppMsg::HitlRequestReceived(request) => {
            info!(request_id = %request.request_id, "HITL request received");
            model.add_hitl_request(request);
            model.switch_to_hitl_layout();
        }

        AppMsg::HitlReviewOpen(request_id) => {
            info!(request_id = %request_id, "Opening HITL review");
            if let Some(request) = model.remove_hitl_request(&request_id) {
                model.current_hitl_request = Some(request);
                model.focused_component = crate::message::ComponentId::HitlReview;
            } else {
                warn!(request_id = %request_id, "HITL request not found");
            }
        }

        AppMsg::HitlReviewClose => {
            info!("Closing HITL review");
            model.current_hitl_request = None;
            model.switch_to_normal_layout();
        }

        AppMsg::HitlDecisionMade(request_id, decision) => {
            info!(request_id = %request_id, decision = ?decision, "HITL decision made");
            model.current_hitl_request = None;
            if model.hitl_requests.is_empty() {
                model.switch_to_normal_layout();
            }
        }

        AppMsg::HitlDecisionSent(request_id) => {
            info!(request_id = %request_id, "HITL decision sent successfully");
            model.set_status_message(StatusSeverity::Info,
                format!("HITL decision sent for {}", request_id));
        }

        AppMsg::HitlDecisionFailed(request_id, error) => {
            warn!(request_id = %request_id, error = %error, "HITL decision failed");
            model.set_status_message(StatusSeverity::Error,
                format!("HITL decision failed for {}: {}", request_id, error));
        }

        // ============== UI Navigation Events ==============
        AppMsg::FocusNext => {
            debug!(from = ?model.focused_component, "Focus next component");
            model.focus_next_component();
            debug!(to = ?model.focused_component, "Focused next component");
        }

        AppMsg::FocusPrevious => {
            debug!(from = ?model.focused_component, "Focus previous component");
            model.focus_previous_component();
            debug!(to = ?model.focused_component, "Focused previous component");
        }

        AppMsg::FocusComponent(component_id) => {
            debug!(from = ?model.focused_component, to = ?component_id, "Focus specific component");
            model.focused_component = component_id;
        }

        AppMsg::HelpToggle => {
            debug!(show_help = !model.show_help, "Toggle help");
            model.toggle_help();
        }

        // ============== Layout Events ==============
        AppMsg::LayoutNormal => {
            info!("Switching to normal layout");
            model.switch_to_normal_layout();
        }

        AppMsg::LayoutHitlReview => {
            info!("Switching to HITL review layout");
            model.switch_to_hitl_layout();
        }

        // ============== Error Events ==============
        AppMsg::ErrorOccurred(error) => {
            warn!(error = %error, "Error occurred");
            model.set_status_message(StatusSeverity::Error, error);
        }

        AppMsg::StatusMessage(severity, message) => {
            debug!(severity = ?severity, message = %message, "Status message");
            model.set_status_message(severity, message);
        }
    }

    debug!(effects_count = effects.len(), "Update complete, returning effects");
    Ok(effects)
}

/// Handle status events from WebSocket and update timeline
#[instrument(level = "debug", skip(model), fields(
    event_type = ?event.event,
    execution_id = %event.execution_id,
    source = ?event.source,
))]
fn handle_status_event(model: &mut AppModel, event: StatusEvent) -> Result<()> {
    debug!("Processing status event");

    match &event.event {
        EventType::ExecutionStarted { query } => {
            info!(query = %query, "Execution started");
            let root = model.timeline_tree.add_root(
                event.execution_id.clone(),
                format!("Query: {}", query)
            );
            root.start();
        }

        EventType::AgentStarted { context_size } => {
            if let EventSource::Agent { agent_id, agent_type, task_id: _ } = &event.source {
                info!(agent_id = %agent_id, agent_type = ?agent_type, context_size, "Agent started");
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
                info!(agent_id = %agent_id, "Agent completed");
                if let Some(node) = model.timeline_tree.find_node_mut(agent_id) {
                    node.complete();
                }
            }
        }

        EventType::AgentFailed { error } => {
            if let EventSource::Agent { agent_id, .. } = &event.source {
                warn!(agent_id = %agent_id, error = %error, "Agent failed");
                if let Some(node) = model.timeline_tree.find_node_mut(agent_id) {
                    node.fail(Some(error.clone()));
                }
            }
        }

        EventType::ToolStarted { args } => {
            if let EventSource::Tool { agent_id, tool_name } = &event.source {
                info!(agent_id = %agent_id, tool_name = %tool_name, "Tool started");
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
                info!(agent_id = %agent_id, tool_name = %tool_name, "Tool completed");
                let tool_id = format!("{}:{}", agent_id, tool_name);
                if let Some(node) = model.timeline_tree.find_node_mut(&tool_id) {
                    node.complete();
                }
            }
        }

        EventType::ToolFailed { error } => {
            if let EventSource::Tool { agent_id, tool_name } = &event.source {
                warn!(agent_id = %agent_id, tool_name = %tool_name, error = %error, "Tool failed");
                let tool_id = format!("{}:{}", agent_id, tool_name);
                if let Some(node) = model.timeline_tree.find_node_mut(&tool_id) {
                    node.fail(Some(error.clone()));
                }
            }
        }

        EventType::ExecutionCompleted { result: _ } => {
            info!(execution_id = %event.execution_id, "Execution completed");
            if let Some(root) = model.timeline_tree.find_node_mut(&event.execution_id) {
                root.complete();
            }
        }

        EventType::ExecutionFailed { error } => {
            warn!(execution_id = %event.execution_id, error = %error, "Execution failed");
            if let Some(root) = model.timeline_tree.find_node_mut(&event.execution_id) {
                root.fail(Some(error.clone()));
            }
        }

        _ => {
            debug!(event_type = ?event.event, "Unhandled event type");
        }
    }

    Ok(())
}
