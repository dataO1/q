//! Comprehensive event logging utilities for debugging
//!
//! This module provides structured logging helpers for all application events,
//! making debugging easier by providing consistent, searchable log entries.

use tracing::{debug, info, trace, warn};
use crate::{
    application::state::AppModel,
    message::{ComponentId, UserEvent},
};
use crossterm::event::{KeyCode, KeyModifiers};
use serde_json::Value;

/// Structured event logger for comprehensive debugging
pub struct EventLogger;

impl EventLogger {
    /// Log keyboard events with full context
    pub fn log_keyboard_event(
        key: KeyCode,
        modifiers: KeyModifiers,
        component: &ComponentId,
        processed: bool,
    ) {
        info!(
            event = "keyboard",
            ?key,
            ?modifiers,
            ?component,
            processed,
            "Keyboard event received"
        );
    }

    /// Log TUIRealm events
    pub fn log_tuirealm_event(event_type: &str, component: Option<&ComponentId>) {
        debug!(
            event = "tuirealm",
            event_type,
            ?component,
            "TUIRealm event processed"
        );
    }

    /// Log state changes with before/after comparison
    pub fn log_state_change(before: &AppModel, after: &AppModel, trigger: &str) {
        let state_changed = before.focused_component != after.focused_component
            || before.layout_mode != after.layout_mode
            || before.show_help != after.show_help;

        if state_changed {
            debug!(
                event = "state_change",
                trigger,
                focused_before = ?before.focused_component,
                focused_after = ?after.focused_component,
                layout_before = ?before.layout_mode,
                layout_after = ?after.layout_mode,
                help_toggled = (before.show_help != after.show_help),
                "State changed"
            );
        } else {
            trace!(
                event = "state_no_change",
                trigger,
                "State update had no effect"
            );
        }
    }

    /// Log render decisions and performance
    pub fn log_render_decision(
        needs_render: bool,
        render_time_ms: Option<u128>,
    ) {
        trace!(
            event = "render_decision",
            needs_render,
            ?render_time_ms,
            "Render decision made"
        );
    }

    /// Log message processing with queue metrics
    pub fn log_message_processing(
        msg: &UserEvent,
        queue_size_before: usize,
        queue_size_after: usize,
        processing_time_ms: Option<u128>,
    ) {
        debug!(
            event = "message_processed",
            message_type = %format!("{:?}", msg).split('(').next().unwrap_or("Unknown"),
            queue_size_before,
            queue_size_after,
            ?processing_time_ms,
            "Message processed"
        );

        // Log specific message details based on type
        match msg {
            UserEvent::QuerySubmitted(query) => {
                info!(event = "query_submitted", "Query execution started");
            }
            UserEvent::FocusNext | UserEvent::FocusPrevious => {
                debug!(event = "focus_change", action = ?msg, "Focus change requested");
            }
            // UserEvent::StatusEventReceived(event) => {
            //     info!(
            //         event = "status_event",
            //         event_data = ?event,
            //         "Status event received"
            //     );
            // }
            UserEvent::WebSocketConnected(subscription_id) => {
                info!(
                    event = "websocket",
                    status = "connected",
                    subscription_id = %subscription_id,
                    "WebSocket connection established"
                );
            }
            UserEvent::WebSocketDisconnected => {
                warn!(event = "websocket", status = "disconnected", "WebSocket connection lost");
            }
            UserEvent::Tick => {
                trace!(event = "tick", "Animation tick processed");
            }
            _ => {} // Other messages logged at debug level above
        }
    }

    /// Log focus changes with component details
    pub fn log_focus_change(from: &ComponentId, to: &ComponentId, success: bool) {
        info!(
            event = "focus_change",
            from = ?from,
            to = ?to,
            success,
            "Component focus changed"
        );
    }

    /// Log component synchronization
    pub fn log_component_sync(component: &ComponentId, sync_type: &str, success: bool, error: Option<&str>) {
        if success {
            debug!(
                event = "component_sync",
                ?component,
                sync_type,
                "Component synchronized with model"
            );
        } else {
            warn!(
                event = "component_sync_error",
                ?component,
                sync_type,
                ?error,
                "Component synchronization failed"
            );
        }
    }

    /// Log network events (WebSocket, HTTP)
    pub fn log_network_event(
        event_type: &str,
        url: Option<&str>,
        success: bool,
        latency_ms: Option<u128>,
        error: Option<&str>,
    ) {
        if success {
            info!(
                event = "network",
                event_type,
                ?url,
                ?latency_ms,
                "Network operation completed"
            );
        } else {
            warn!(
                event = "network_error",
                event_type,
                ?url,
                ?error,
                "Network operation failed"
            );
        }
    }

    /// Log performance metrics
    pub fn log_performance_metrics(
        operation: &str,
        duration_ms: u128,
        memory_kb: Option<usize>,
        queue_sizes: Option<&[(&str, usize)]>,
    ) {
        debug!(
            event = "performance",
            operation,
            duration_ms,
            ?memory_kb,
            ?queue_sizes,
            "Performance metrics recorded"
        );

        // Warn on slow operations
        if duration_ms > 100 {
            warn!(
                event = "performance_warning",
                operation,
                duration_ms,
                "Operation took longer than 100ms"
            );
        }
    }

    /// Log application lifecycle events
    pub fn log_lifecycle_event(phase: &str, details: Option<Value>) {
        info!(
            event = "lifecycle",
            phase,
            ?details,
            "Application lifecycle event"
        );
    }

    /// Log error events with full context
    pub fn log_error(
        operation: &str,
        error: &anyhow::Error,
        component: Option<&ComponentId>,
        recoverable: bool,
    ) {
        warn!(
            event = "error",
            operation,
            error_msg = %error,
            error_chain = ?error.chain().collect::<Vec<_>>(),
            ?component,
            recoverable,
            "Error occurred"
        );
    }

    /// Log debug checkpoints with arbitrary context
    pub fn log_debug_checkpoint(checkpoint: &str, context: Value) {
        trace!(
            event = "debug_checkpoint",
            checkpoint,
            context = ?context,
            "Debug checkpoint reached"
        );
    }
}

/// Helper macro for timing operations and logging performance
#[macro_export]
macro_rules! time_operation {
    ($operation:expr, $code:block) => {{
        let start = std::time::Instant::now();
        let result = $code;
        let duration = start.elapsed().as_millis();

        $crate::utils::event_logger::EventLogger::log_performance_metrics(
            $operation,
            duration,
            None,
            None,
        );

        result
    }};
}

/// Helper macro for logging state changes
#[macro_export]
macro_rules! log_state_change {
    ($before:expr, $after:expr, $trigger:expr) => {
        $crate::utils::event_logger::EventLogger::log_state_change(
            $before,
            $after,
            $trigger
        );
    };
}
