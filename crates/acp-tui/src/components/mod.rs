//! TUIRealm Components for the ACP TUI application
//!
//! This module contains all UI components built with TUIRealm's Elm-style architecture.

pub mod realm;

// Re-export all TUIRealm components
pub use realm::{
    TimelineRealmComponent, QueryInputRealmComponent, StatusLineRealmComponent,
    HitlReviewRealmComponent, HitlQueueRealmComponent,
};

// Legacy types for compatibility
use chrono::{DateTime, Utc};
use crate::message::StatusSeverity;

/// A status message to display
#[derive(Debug, Clone)]
pub struct StatusMessage {
    /// The severity level of this message
    pub severity: StatusSeverity,
    /// The message text to display
    pub message: String,
    /// When this message was created
    pub timestamp: DateTime<Utc>,
    /// Optional error code for programmatic handling
    pub error_code: Option<String>,
}