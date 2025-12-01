//! Application state (Model in Elm architecture)

use chrono::{DateTime, Utc};
use crate::{
    components::realm::status_line::ConnectionState,
    components::StatusMessage,
    message::{ComponentId, StatusSeverity, LayoutMode},
    models::tree::TimelineTree,
    client::types::HitlApprovalRequest,
};

/// Component-specific dirty flags for optimized updates
#[derive(Debug, Clone, Default)]
pub struct ComponentDirtyFlags {
    /// Timeline component needs update
    pub timeline: bool,
    /// Query input component needs update
    pub query_input: bool,
    /// Status line component needs update
    pub status_line: bool,
    /// HITL queue component needs update
    pub hitl_queue: bool,
    /// HITL review modal needs update
    pub hitl_review: bool,
    /// Help overlay needs update
    pub help: bool,
}

impl ComponentDirtyFlags {
    /// Mark all components as needing updates
    pub fn mark_all_dirty(&mut self) {
        self.timeline = true;
        self.query_input = true;
        self.status_line = true;
        self.hitl_queue = true;
        self.hitl_review = true;
        self.help = true;
    }
    
    /// Clear all dirty flags
    pub fn clear_all(&mut self) {
        *self = Self::default();
    }
    
    /// Check if any component is dirty
    pub fn any_dirty(&self) -> bool {
        self.timeline || self.query_input || self.status_line || 
        self.hitl_queue || self.hitl_review || self.help
    }
}

/// Core application state following Elm's Model pattern
#[derive(Debug, Clone)]
pub struct AppModel {
    /// Current client ID
    pub client_id: String,
    
    /// Connection state to ACP server
    pub connection_state: ConnectionState,
    
    /// Component dirty flags for smart updates
    pub component_dirty_flags: ComponentDirtyFlags,
    
    /// Current status message to display
    pub status_message: Option<StatusMessage>,
    
    /// Currently focused component
    pub focused_component: ComponentId,
    
    /// Current layout mode
    pub layout_mode: LayoutMode,
    
    /// Whether help overlay is visible
    pub show_help: bool,
    
    /// Timeline data
    pub timeline_tree: TimelineTree,
    
    /// HITL requests queue
    pub hitl_requests: Vec<HitlApprovalRequest>,
    
    /// Currently selected HITL request for review
    pub current_hitl_request: Option<HitlApprovalRequest>,
    
    /// Query input text
    pub query_text: String,
    
    /// Last execution timestamp
    pub last_execution_time: Option<DateTime<Utc>>,
    
    /// Scroll position in timeline
    pub timeline_scroll: usize,
    
    /// Animation frame counter
    pub animation_frame: usize,
}

impl AppModel {
    /// Create a new application model
    pub fn new(client_id: String) -> Self {
        let mut component_dirty_flags = ComponentDirtyFlags::default();
        component_dirty_flags.mark_all_dirty(); // Initial render needs all components
        
        Self {
            client_id,
            connection_state: ConnectionState::Disconnected,
            component_dirty_flags,
            status_message: None,
            focused_component: ComponentId::QueryInput, // Start with input focused
            layout_mode: LayoutMode::Normal,
            show_help: false,
            timeline_tree: TimelineTree::new(),
            hitl_requests: Vec::new(),
            current_hitl_request: None,
            query_text: String::new(),
            last_execution_time: None,
            timeline_scroll: 0,
            animation_frame: 0,
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
    
    /// Focus next component in tab order
    pub fn focus_next_component(&mut self) {
        self.focused_component = match self.focused_component {
            ComponentId::QueryInput => ComponentId::Timeline,
            ComponentId::Timeline => ComponentId::HitlQueue,
            ComponentId::HitlQueue => ComponentId::HitlReview,
            ComponentId::HitlReview => ComponentId::QueryInput,
            ComponentId::StatusLine => ComponentId::QueryInput, // Status line not focusable
            ComponentId::Help => ComponentId::QueryInput, // Help overlay closes
        };
    }
    
    /// Focus previous component in tab order
    pub fn focus_previous_component(&mut self) {
        self.focused_component = match self.focused_component {
            ComponentId::QueryInput => ComponentId::HitlReview,
            ComponentId::Timeline => ComponentId::QueryInput,
            ComponentId::HitlQueue => ComponentId::Timeline,
            ComponentId::HitlReview => ComponentId::HitlQueue,
            ComponentId::StatusLine => ComponentId::QueryInput, // Status line not focusable
            ComponentId::Help => ComponentId::QueryInput, // Help overlay closes
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
    
    /// Clear query text
    pub fn clear_query(&mut self) {
        self.query_text.clear();
    }
    
    /// Set query text
    pub fn set_query(&mut self, text: String) {
        self.query_text = text;
    }
    
    /// Get scroll info for timeline
    pub fn get_scroll_info(&self) -> (usize, usize) {
        let total_lines = self.timeline_tree.render_lines().len();
        (self.timeline_scroll, total_lines)
    }
    
    /// Scroll timeline up
    pub fn scroll_timeline_up(&mut self) {
        self.timeline_scroll = self.timeline_scroll.saturating_sub(1);
    }
    
    /// Scroll timeline down
    pub fn scroll_timeline_down(&mut self) {
        self.timeline_scroll = self.timeline_scroll.saturating_add(1);
    }
    
    /// Advance animation frame
    pub fn tick_animation(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
        self.timeline_tree.advance_animations();
    }
    
    /// Toggle help visibility
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        if self.show_help {
            self.focused_component = ComponentId::Help;
        }
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