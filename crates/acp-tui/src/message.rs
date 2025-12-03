//! Application message types for TUIRealm Elm architecture
//!
//! All state changes in the application happen through these messages,
//! following the Elm architecture pattern supported by TUIRealm.

use crate::client::types::{StatusEvent, ProjectScope};

// Implement PartialEq for the types that don't have it
impl PartialEq for StatusEvent {
    fn eq(&self, other: &Self) -> bool {
        self.conversation_id == other.conversation_id
            && self.timestamp == other.timestamp
            // Skip source comparison for now since EventSource doesn't implement PartialEq
    }
}


impl Eq for StatusEvent {}
impl Eq for ProjectScope {}

impl PartialEq for ProjectScope {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root && self.current_file == other.current_file
    }
}

/// Main application messages for TUIRealm
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum APIEvent {
    // ============== System Events ==============
    /// Application should quit

    // ============== Timeline Events ==============
    /// Status event received from WebSocket (using generated type)
    StatusEventReceived(StatusEvent),

    /// Query execution started
    QueryExecutionStarted(String), // query text
    /// Query execution completed
    QueryExecutionCompleted(String), // result
    /// Query execution failed
    QueryExecutionFailed(String), // error

    // ============== Error Events ==============
    /// General error occurred
    ErrorOccurred(String),
    /// WebSocket connected successfully with subscription
    WebSocketConnected(String), // subscription_id
    /// WebSocket disconnected
    WebSocketDisconnected,
    /// Connection failed with error
    ConnectionFailed(String), // error message
    /// Display status message
    StatusMessage(StatusSeverity, String),
}

/// Component identifiers for TUIRealm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentId {
    /// Root component showing execution tree
    Root,
    /// Timeline component showing execution tree
    Timeline,
    /// Query input text area
    QueryInput,
    /// Status line at bottom
    StatusLine,
    /// HITL review window
    HitlReview,
    /// Help overlay
    Help,
}

impl ComponentId {
    /// Convert to string for TUIRealm
    pub fn as_str(&self) -> &'static str {
        match self {
            ComponentId::Timeline => "timeline",
            ComponentId::QueryInput => "query_input",
            ComponentId::StatusLine => "status_line",
            ComponentId::HitlReview => "hitl_review",
            ComponentId::Help => "help",
            ComponentId::Root => "root",
        }
    }
}

impl From<ComponentId> for tuirealm::AttrValue {
    fn from(id: ComponentId) -> Self {
        tuirealm::AttrValue::String(id.as_str().to_string())
    }
}

/// Status message severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl StatusSeverity {
    /// Get the color for this severity
    pub fn color(&self) -> ratatui::style::Color {
        match self {
            StatusSeverity::Info => ratatui::style::Color::Blue,
            StatusSeverity::Warning => ratatui::style::Color::Yellow,
            StatusSeverity::Error => ratatui::style::Color::Red,
            StatusSeverity::Critical => ratatui::style::Color::LightRed,
        }
    }

    /// Get the symbol for this severity
    pub fn symbol(&self) -> &'static str {
        match self {
            StatusSeverity::Info => "ℹ",
            StatusSeverity::Warning => "⚠",
            StatusSeverity::Error => "✗",
            StatusSeverity::Critical => "🔴",
        }
    }
}

/// Component messages generated from keyboard events and UI interactions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserEvent {
    Quit,
    Tick,
    /// Move focus to next component
    FocusNext,
    /// Move focus to previous component
    FocusPrevious,
    /// Toggle help overlay
    HelpToggle,

    // ============== Connection Events ==============
    /// Start connection flow (create subscription)
    StartConnection,
    /// WebSocket connected successfully with subscription
    WebSocketConnected(String), // subscription_id
    /// Subscription was resumed (existing one found)
    SubscriptionResumed(String), // subscription_id
    /// WebSocket disconnected
    WebSocketDisconnected,
    /// Connection failed with error
    ConnectionFailed(String), // error message

    // ============== Query Events ==============
    /// Query was submitted for execution
    QuerySubmitted(String),
    /// Query execution started
    QueryExecutionStarted(String), // query text
    /// Query execution completed
    QueryExecutionCompleted(String), // result
    /// Query execution failed
    QueryExecutionFailed(String), // error
    // ============== HITL Events ==============
    HitlDecisionSubmit{
        id: String,
        approved: bool,
        modified_content: Option<String>,
        reasoning: Option<String>,
    },

    // ============== UI Navigation Events ==============
    /// Change focus to next component
    /// Focus specific component
    FocusComponent(ComponentId),
    // ============== Error Events ==============
    /// General error occurred
    ErrorOccurred(String),
    /// Display status message
    StatusMessage(StatusSeverity, String),
}

/// User events (currently none, but required by TUIRealm)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoUserEvent {}
