//! Structured Audit Logger with OpenTelemetry integration

use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry::{global, KeyValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, span, warn, Level};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub agent_id: String,
    pub task_id: String,
    pub action: String,
    pub risk_level: String,
    pub decision: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug)]
pub struct AuditLogger;

impl AuditLogger {
    /// Log audit event with OpenTelemetry span
    pub fn log(event: AuditEvent) {
        let span = span!(
            Level::INFO,
            "audit.event",
            event_id = %event.event_id,
            task_id = %event.task_id,
            agent_id = %event.agent_id,
            action = %event.action,
            risk_level = %event.risk_level,
        );

        let _enter = span.enter();

        // Get current OpenTelemetry context and add event
        let cx = opentelemetry::Context::current();
        let span = cx.span();

        span.add_event(
            "audit_event",
            vec![
                KeyValue::new("event.id", event.event_id.clone()),
                KeyValue::new("event.action", event.action.clone()),
                KeyValue::new("event.risk_level", event.risk_level.clone()),
                KeyValue::new("event.decision", event.decision.clone()),
            ],
        );

        info!(target: "audit", "Audit Event: {:?}", event);
    }

    /// Log warning audit event
    pub fn warn(event: AuditEvent) {
        let span = span!(
            Level::WARN,
            "audit.warning",
            event_id = %event.event_id,
            task_id = %event.task_id,
        );

        let _enter = span.enter();
        warn!(target: "audit", "Audit Warning: {:?}", event);
    }

    /// Log error audit event
    pub fn error(event: AuditEvent) {
        let span = span!(
            Level::ERROR,
            "audit.error",
            event_id = %event.event_id,
            task_id = %event.task_id,
        );

        let _enter = span.enter();
        error!(target: "audit", "Audit Error: {:?}", event);
    }
}
