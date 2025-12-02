//! Application state (Model in Elm architecture)

use chrono::{DateTime, Utc};
use tuirealm::{AttrValue, Attribute};
use crate::{
    components::realm::status_line::ConnectionState,
    components::StatusMessage,
    message::{ComponentId, StatusSeverity, LayoutMode},
    models::tree::TimelineTree,
    client::types::HitlApprovalRequest,
};

/// Core application state following Elm's Model pattern
#[derive(Debug, Clone)]
pub struct AppModel {
    /// Current client ID
    pub client_id: String,

    /// Connection state to ACP server
    pub connection_state: ConnectionState,

    /// Current status message to display
    pub status_message: Option<StatusMessage>,

    /// Currently focused component
    pub focused_component: ComponentId,

    /// Current layout mode
    pub layout_mode: LayoutMode,

    /// Whether help overlay is visible
    pub show_help: bool,

    /// HITL requests queue
    pub hitl_requests: Vec<HitlApprovalRequest>,

    /// Currently selected HITL request for review
    pub current_hitl_request: Option<HitlApprovalRequest>,

    /// Last execution timestamp
    pub last_execution_time: Option<DateTime<Utc>>,
}

impl AppModel {
    /// Create a new application model
    pub fn new(client_id: String) -> Self {

        Self {
            client_id,
            connection_state: ConnectionState::Disconnected,
            status_message: None,
            focused_component: ComponentId::QueryInput, // Start with input focused
            layout_mode: LayoutMode::Normal,
            show_help: false,
            hitl_requests: Vec::new(),
            current_hitl_request: None,
            last_execution_time: None,
        }
    }

    /// Set status message
    pub fn set_status_message(&mut self, severity: StatusSeverity, message: String) {
        self.status_message = Some(StatusMessage {
            severity,
            message,
            timestamp: Utc::now(),
            error_code: None,
        });
    }

    /// Clear status message
    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    /// Set connection state
    pub fn set_connection_state(&mut self, state: ConnectionState) {
        self.connection_state = state;
    }

    /// Focus next component in tab order (context-aware)
    pub fn focus_next_component(&mut self) {
        self.focused_component = match self.layout_mode {
            LayoutMode::Normal => {
                // Normal mode: only QueryInput and Timeline are focusable
                match self.focused_component {
                    ComponentId::QueryInput => ComponentId::Timeline,
                    ComponentId::Timeline => ComponentId::QueryInput,
                    // If somehow on a non-normal component, reset to QueryInput
                    _ => ComponentId::QueryInput,
                }
            }
            LayoutMode::HitlReview => {
                // HITL mode: QueryInput, Timeline, HitlQueue, HitlReview
                match self.focused_component {
                    ComponentId::Timeline => ComponentId::HitlQueue,
                    ComponentId::HitlQueue => {
                        if self.current_hitl_request.is_some() {
                            ComponentId::HitlReview
                        } else {
                            ComponentId::Timeline
                        }
                    }
                    ComponentId::HitlReview => ComponentId::Timeline,
                    _ => ComponentId::Timeline,
                }
            }
        };
    }

    /// Focus previous component in tab order (context-aware)
    pub fn focus_previous_component(&mut self) {
        self.focused_component = match self.layout_mode {
            LayoutMode::Normal => {
                // Normal mode: only QueryInput and Timeline are focusable
                match self.focused_component {
                    ComponentId::QueryInput => ComponentId::Timeline,
                    ComponentId::Timeline => ComponentId::QueryInput,
                    // If somehow on a non-normal component, reset to QueryInput
                    _ => ComponentId::QueryInput,
                }
            }
            LayoutMode::HitlReview => {
                // HITL mode: reverse order
                match self.focused_component {
                    ComponentId::Timeline => {
                        if self.current_hitl_request.is_some() {
                            ComponentId::HitlReview
                        } else {
                            ComponentId::HitlQueue
                        }
                    }
                    ComponentId::HitlQueue => ComponentId::Timeline,
                    ComponentId::HitlReview => ComponentId::HitlQueue,
                    _ => ComponentId::Timeline,
                }
            }
        };
    }

    /// Add HITL request to queue
    pub fn add_hitl_request(&mut self, request: HitlApprovalRequest) {
        self.hitl_requests.push(request);
    }

    /// Remove HITL request from queue
    pub fn remove_hitl_request(&mut self, request_id: &str) -> Option<HitlApprovalRequest> {
        self.hitl_requests
            .iter()
            .position(|r| r.request_id == request_id)
            .map(|index| self.hitl_requests.remove(index))
    }

    /// Toggle help visibility
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Switch to HITL review layout
    pub fn switch_to_hitl_layout(&mut self) {
        self.layout_mode = LayoutMode::HitlReview;
    }

    /// Switch to normal layout
    pub fn switch_to_normal_layout(&mut self) {
        self.layout_mode = LayoutMode::Normal;
    }
}
