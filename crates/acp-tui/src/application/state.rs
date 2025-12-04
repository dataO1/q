//! Application state (Model in Elm architecture)

use chrono::{DateTime, Utc};
use tuirealm::{AttrValue, Attribute};
use crate::{
    components::realm::status_line::ConnectionState,
    components::StatusMessage,
    message::{ComponentId, StatusSeverity},
    models::tree::TimelineTree,
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

    /// Whether help overlay is visible
    pub show_help: bool,

    /// Whether hitl overlay is visible
    pub show_hitl_popup: bool,

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
            show_help: false,
            show_hitl_popup: false,
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

    /// open popup model
    pub fn hitl_popup_open(&mut self){
        self.show_hitl_popup = true;
        self.focused_component = ComponentId::HitlReview;
    }

    /// close popup model
    pub fn hitl_popup_close(&mut self){
        self.show_hitl_popup = false;
        self.focused_component = ComponentId::Timeline;
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
        self.focus_previous_component()
    }

    /// Focus previous component in tab order (context-aware)
    pub fn focus_previous_component(&mut self) {
        self.focused_component =
            match self.focused_component {
                ComponentId::QueryInput => ComponentId::Timeline,
                ComponentId::Timeline => ComponentId::QueryInput,
                // If somehow on a non-normal component, reset to QueryInput
                _ => ComponentId::QueryInput,
            }
    }

    /// Toggle help visibility
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

}
