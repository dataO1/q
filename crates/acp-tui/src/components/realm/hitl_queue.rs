//! HitlQueue TUIRealm component
//!
//! Displays a list of pending HITL approval requests.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use tuirealm::{
    command::{Cmd, CmdResult, Direction as MoveDirection},
    event::{Key, KeyEvent as TuiKeyEvent},
    Component, Event, MockComponent, State, StateValue, AttrValue, Attribute,
};

use crate::client::types::HitlApprovalRequest;
use crate::message::{AppMsg, NoUserEvent};

/// HitlQueue component using TUIRealm architecture
pub struct HitlQueueRealmComponent {
    /// List of pending HITL requests
    requests: Vec<HitlApprovalRequest>,
    /// List state for navigation
    list_state: ListState,
    /// Whether this component is focused
    focused: bool,
    /// Whether the queue is visible
    visible: bool,
}

impl HitlQueueRealmComponent {
    /// Create new HITL queue component
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        
        Self {
            requests: Vec::new(),
            list_state,
            focused: false,
            visible: false,
        }
    }
    
    /// Add a new HITL request to the queue
    pub fn add_request(&mut self, request: HitlApprovalRequest) {
        self.requests.push(request);
        
        // If this is the first request and nothing is selected, select it
        if self.requests.len() == 1 {
            self.list_state.select(Some(0));
        }
        
        // Show the queue if we have requests
        if !self.requests.is_empty() {
            self.visible = true;
        }
    }
    
    /// Remove a request by ID  
    pub fn remove_request(&mut self, request_id: &str) -> Option<HitlApprovalRequest> {
        if let Some(index) = self.requests.iter().position(|r| r.agent_id == request_id) {
            let removed = self.requests.remove(index);
            
            // Adjust selection if necessary
            if let Some(selected) = self.list_state.selected() {
                if selected >= self.requests.len() && !self.requests.is_empty() {
                    self.list_state.select(Some(self.requests.len() - 1));
                } else if self.requests.is_empty() {
                    self.list_state.select(None);
                    self.visible = false;
                }
            }
            
            Some(removed)
        } else {
            None
        }
    }
    
    /// Get the currently selected request
    pub fn get_selected_request(&self) -> Option<&HitlApprovalRequest> {
        self.list_state.selected()
            .and_then(|index| self.requests.get(index))
    }
    
    /// Get the number of pending requests
    pub fn request_count(&self) -> usize {
        self.requests.len()
    }
    
    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.requests.is_empty() {
            return;
        }
        
        let selected = self.list_state.selected().unwrap_or(0);
        let new_selected = if selected == 0 {
            self.requests.len() - 1
        } else {
            selected - 1
        };
        self.list_state.select(Some(new_selected));
    }
    
    /// Move selection down
    pub fn select_next(&mut self) {
        if self.requests.is_empty() {
            return;
        }
        
        let selected = self.list_state.selected().unwrap_or(0);
        let new_selected = if selected >= self.requests.len() - 1 {
            0
        } else {
            selected + 1
        };
        self.list_state.select(Some(new_selected));
    }
    
    /// Show the queue
    pub fn show(&mut self) {
        if !self.requests.is_empty() {
            self.visible = true;
        }
    }
    
    /// Hide the queue
    pub fn hide(&mut self) {
        self.visible = false;
    }
    
    /// Is the queue visible?
    pub fn is_visible(&self) -> bool {
        self.visible && !self.requests.is_empty()
    }
    
    /// Format a request for display (static version to avoid borrowing issues)
    fn format_request_static(request: &HitlApprovalRequest, index: usize, selected: bool) -> ListItem {
        // Use available fields from HitlApprovalRequest
        let prefix = if selected { "▶ " } else { "  " };
        let agent_info = format!("{:?} Agent", request.agent_type);
        let context_desc = if request.context.len() > 40 {
            format!("{}...", &request.context[..37])
        } else {
            request.context.clone()
        };
        
        let line = Line::from(vec![
            Span::raw(prefix),
            Span::styled(format!("#{} ", index + 1), Style::default().fg(Color::Gray)),
            Span::styled(
                format!("[{}] ", agent_info),
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            ),
            Span::raw(context_desc),
        ]);
        
        let style = if selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        
        ListItem::new(line).style(style)
    }
}

impl Component<AppMsg, NoUserEvent> for HitlQueueRealmComponent {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<AppMsg> {
        if !self.visible || !self.focused {
            return None;
        }
        
        match ev {
            Event::Keyboard(key_event) => {
                match key_event {
                    TuiKeyEvent { code: Key::Up, .. } => {
                        self.select_previous();
                        None
                    },
                    TuiKeyEvent { code: Key::Down, .. } => {
                        self.select_next();
                        None
                    },
                    TuiKeyEvent { code: Key::Enter, .. } => {
                        // Open selected request for review
                        if let Some(request) = self.get_selected_request() {
                            Some(AppMsg::HitlReviewOpen(request.agent_id.clone()))
                        } else {
                            None
                        }
                    },
                    TuiKeyEvent { code: Key::Esc, .. } | TuiKeyEvent { code: Key::Char('q'), .. } => {
                        self.hide();
                        None
                    },
                    _ => None,
                }
            },
            _ => None,
        }
    }
}

impl MockComponent for HitlQueueRealmComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        if !self.is_visible() {
            return;
        }
        
        let border_style = if self.focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };
        
        let title = format!(" HITL Queue ({}) ", self.requests.len());
        
        // Create list items - avoid borrowing self 
        let selected_index = self.list_state.selected();
        let mut items = Vec::new();
        for (index, request) in self.requests.iter().enumerate() {
            let selected = selected_index == Some(index);
            items.push(Self::format_request_static(request, index, selected));
        }
        
        let list = List::new(items)
            .block(Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            );
        
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
    
    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        match attr {
            Attribute::Focus => Some(AttrValue::Flag(self.focused)),
            Attribute::Display => Some(AttrValue::Flag(self.visible)),
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
            Attribute::Display => {
                if let AttrValue::Flag(visible) = value {
                    if visible {
                        self.show();
                    } else {
                        self.hide();
                    }
                }
            }
            _ => {}
        }
    }
    
    fn state(&self) -> State {
        if let Some(selected) = self.list_state.selected() {
            State::One(StateValue::Usize(selected))
        } else {
            State::None
        }
    }
    
    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Move(MoveDirection::Up) => {
                self.select_previous();
                CmdResult::Changed(self.state())
            },
            Cmd::Move(MoveDirection::Down) => {
                self.select_next();
                CmdResult::Changed(self.state())
            },
            Cmd::Submit => {
                if let Some(request) = self.get_selected_request() {
                    CmdResult::Submit(State::One(StateValue::String(request.request_id.clone())))
                } else {
                    CmdResult::None
                }
            },
            _ => CmdResult::None,
        }
    }
}