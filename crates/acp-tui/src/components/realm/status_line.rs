//! StatusLine TUIRealm component
//!
//! Displays status messages, connection state, and system information.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tuirealm::{
    command::{Cmd, CmdResult},
    Component, Event, MockComponent, State, StateValue, AttrValue, Attribute,
};

use crate::message::{AppMsg, NoUserEvent};
use crate::components::{StatusMessage};

/// Connection state for display
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: usize },
    Failed { error: String },
}

impl ConnectionState {
    /// Get display text for this connection state
    pub fn display_text(&self) -> &str {
        match self {
            ConnectionState::Disconnected => "Disconnected",
            ConnectionState::Connecting => "Connecting...",
            ConnectionState::Connected => "Connected",
            ConnectionState::Reconnecting { .. } => "Reconnecting...",
            ConnectionState::Failed { .. } => "Failed",
        }
    }
    
    /// Get status dot for this connection state
    pub fn status_dot(&self) -> &str {
        match self {
            ConnectionState::Disconnected => "○",
            ConnectionState::Connecting => "◐",
            ConnectionState::Connected => "●",
            ConnectionState::Reconnecting { .. } => "◑",
            ConnectionState::Failed { .. } => "✗",
        }
    }
    
    /// Get color for this connection state
    pub fn color(&self) -> Color {
        match self {
            ConnectionState::Disconnected => Color::Gray,
            ConnectionState::Connecting => Color::Yellow,
            ConnectionState::Connected => Color::Green,
            ConnectionState::Reconnecting { .. } => Color::Yellow,
            ConnectionState::Failed { .. } => Color::Red,
        }
    }
}

/// StatusLine component using TUIRealm architecture
pub struct StatusLineRealmComponent {
    /// Current status message
    current_message: Option<StatusMessage>,
    /// Connection state
    connection_state: ConnectionState,
    /// Client ID
    client_id: String,
    /// Scroll position info (current, total)
    scroll_info: (usize, usize),
    /// Whether this component is focused (usually false for status line)
    focused: bool,
}

impl StatusLineRealmComponent {
    /// Create new status line component
    pub fn new() -> Self {
        Self {
            current_message: None,
            connection_state: ConnectionState::Disconnected,
            client_id: "unknown".to_string(),
            scroll_info: (0, 0),
            focused: false,
        }
    }
    
    /// Set status message
    pub fn set_message(&mut self, message: StatusMessage) {
        self.current_message = Some(message);
    }
    
    /// Clear status message
    pub fn clear_message(&mut self) {
        self.current_message = None;
    }
    
    /// Set connection state
    pub fn set_connection_state(&mut self, state: ConnectionState) {
        self.connection_state = state;
    }
    
    /// Set client ID
    pub fn set_client_id(&mut self, client_id: String) {
        self.client_id = client_id;
    }
    
    /// Set scroll info
    pub fn set_scroll_info(&mut self, current: usize, total: usize) {
        self.scroll_info = (current, total);
    }
    
    /// Format the status line text
    fn format_status_text(&self) -> String {
        // Get status message text
        let status_text = if let Some(ref msg) = self.current_message {
            &msg.message
        } else {
            "Ready"
        };
        
        // Format client ID (abbreviated)
        let client_display = if self.client_id.len() > 12 {
            format!("{}..{}", &self.client_id[..8], &self.client_id[self.client_id.len()-4..])
        } else {
            self.client_id.clone()
        };
        
        // Build complete status line
        format!(
            " {} | Scroll: {}/{} | Connection: {} {} | Client: {} | Help: ?",
            status_text,
            self.scroll_info.0,
            self.scroll_info.1,
            self.connection_state.status_dot(),
            self.connection_state.display_text(),
            client_display,
        )
    }
    
    /// Get the appropriate color for the status line
    fn get_status_color(&self) -> Color {
        if let Some(ref msg) = self.current_message {
            msg.severity.color()
        } else {
            self.connection_state.color()
        }
    }
}

impl Component<AppMsg, NoUserEvent> for StatusLineRealmComponent {
    fn on(&mut self, _ev: Event<NoUserEvent>) -> Option<AppMsg> {
        // Status line typically doesn't handle input events
        None
    }
}

impl MockComponent for StatusLineRealmComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let status_text = self.format_status_text();
        let color = self.get_status_color();
        
        let style = Style::default()
            .fg(color)
            .add_modifier(if self.current_message.is_some() { 
                Modifier::BOLD 
            } else { 
                Modifier::empty() 
            });
        
        let paragraph = Paragraph::new(status_text)
            .style(style)
            .block(Block::default().borders(Borders::NONE));
        
        frame.render_widget(paragraph, area);
    }
    
    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        match attr {
            Attribute::Focus => Some(AttrValue::Flag(self.focused)),
            Attribute::Text => Some(AttrValue::String(self.format_status_text())),
            _ => None,
        }
    }
    
    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match attr {
            Attribute::Focus => {
                if let AttrValue::Flag(focused) = value {
                    self.focused = focused;
                }
            }
            _ => {}
        }
    }
    
    fn state(&self) -> State {
        State::One(StateValue::String(self.format_status_text()))
    }
    
    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        // Status line typically doesn't perform commands
        CmdResult::None
    }
}