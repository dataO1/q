//! Application message types for TUIRealm Elm architecture
//!
//! All state changes in the application happen through these messages,
//! following the Elm architecture pattern supported by TUIRealm.

use crate::client::types::{StatusEvent, HitlApprovalRequest, HitlDecisionRequest, ProjectScope};

// Implement PartialEq for the types that don't have it
impl PartialEq for StatusEvent {
    fn eq(&self, other: &Self) -> bool {
        self.execution_id == other.execution_id
            && self.timestamp == other.timestamp
            // Skip source comparison for now since EventSource doesn't implement PartialEq
    }
}

impl PartialEq for HitlApprovalRequest {
    fn eq(&self, other: &Self) -> bool {
        self.agent_id == other.agent_id
            && self.agent_type == other.agent_type
            && self.context == other.context
    }
}

impl PartialEq for HitlDecisionRequest {
    fn eq(&self, other: &Self) -> bool {
        // Compare based on all fields since this is a struct, not an enum
        self.decision == other.decision
            && self.modified_content == other.modified_content
            && self.request_id == other.request_id
            && self.reason == other.reason
    }
}

impl Eq for StatusEvent {}
impl Eq for HitlApprovalRequest {}
impl Eq for HitlDecisionRequest {}
impl Eq for ProjectScope {}

impl PartialEq for ProjectScope {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root && self.current_file == other.current_file
    }
}

/// Main application messages for TUIRealm
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMsg {
    // ============== System Events ==============
    /// Application should quit
    Quit,
    /// Terminal was resized
    TerminalResized(u16, u16),
    /// Animation/refresh tick
    Tick,

    // ============== Connection Events ==============
    /// Start connection flow (create subscription)
    StartConnection,
    /// Subscription was created successfully
    SubscriptionCreated(String), // subscription_id
    /// Subscription was resumed (existing one found)
    SubscriptionResumed(String), // subscription_id
    /// WebSocket connected successfully
    WebSocketConnected,
    /// WebSocket disconnected
    WebSocketDisconnected,
    /// Connection failed with error
    ConnectionFailed(String), // error message

    // ============== Query Events ==============
    /// Query input text changed
    QueryInputChanged(String),
    /// Query was submitted for execution
    QuerySubmitted,
    /// Query execution started
    QueryExecutionStarted(String), // query text
    /// Query execution completed
    QueryExecutionCompleted(String), // result
    /// Query execution failed
    QueryExecutionFailed(String), // error

    // ============== Timeline Events ==============
    /// Status event received from WebSocket (using generated type)
    StatusEventReceived(StatusEvent),
    /// Scroll timeline up
    TimelineScrollUp,
    /// Scroll timeline down
    TimelineScrollDown,
    /// Toggle node expansion/collapse
    TimelineNodeToggle(String), // node_id
    /// Clear timeline
    TimelineClear,

    // ============== HITL Events ==============
    /// HITL approval request received (using generated type)
    HitlRequestReceived(HitlApprovalRequest),
    /// Open HITL review window for specific request
    HitlReviewOpen(String), // request_id
    /// Close HITL review window
    HitlReviewClose,
    /// HITL decision was made (using generated type)
    HitlDecisionMade(String, HitlDecisionRequest), // request_id, decision
    /// HITL decision was sent successfully
    HitlDecisionSent(String), // request_id
    /// HITL decision sending failed
    HitlDecisionFailed(String, String), // request_id, error

    // ============== UI Navigation Events ==============
    /// Change focus to next component
    FocusNext,
    /// Change focus to previous component
    FocusPrevious,
    /// Focus specific component
    FocusComponent(ComponentId),
    /// Toggle help overlay
    HelpToggle,

    // ============== Layout Events ==============
    /// Switch to normal layout (timeline + query)
    LayoutNormal,
    /// Switch to HITL review layout
    LayoutHitlReview,

    // ============== Error Events ==============
    /// General error occurred
    ErrorOccurred(String),
    /// Display status message
    StatusMessage(StatusSeverity, String),
}

/// Component identifiers for TUIRealm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentId {
    /// Timeline component showing execution tree
    Timeline,
    /// Query input text area
    QueryInput,
    /// Status line at bottom
    StatusLine,
    /// HITL queue list
    HitlQueue,
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
            ComponentId::HitlQueue => "hitl_queue",
            ComponentId::HitlReview => "hitl_review",
            ComponentId::Help => "help",
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

/// Layout modes for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Normal layout: Timeline + QueryInput + StatusLine
    Normal,
    /// HITL review layout: HitlQueue + HitlReview + Timeline (smaller)
    HitlReview,
}

impl Default for LayoutMode {
    fn default() -> Self {
        LayoutMode::Normal
    }
}

/// Component messages generated from keyboard events and UI interactions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentMsg {
    /// Application should quit
    AppQuit,
    /// Query was submitted for execution
    QuerySubmit,
    /// Move focus to next component
    FocusNext,
    /// Move focus to previous component
    FocusPrevious,
    /// Toggle help overlay
    HelpToggle,
    /// Clear timeline
    TimelineClear,
    /// Scroll timeline up
    TimelineScrollUp,
    /// Scroll timeline down
    TimelineScrollDown,
    /// Submit HITL decision
    HitlSubmitDecision,
    /// Cancel HITL review
    HitlCancelReview,
    /// Open HITL review
    HitlOpenReview,
    /// No action (default for unhandled events)
    None,
}

/// User events (currently none, but required by TUIRealm)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoUserEvent {}
