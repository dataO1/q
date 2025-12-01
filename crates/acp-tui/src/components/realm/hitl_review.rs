//! HitlReview TUIRealm component
//!
//! Modal component for reviewing HITL approval requests.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Key, KeyEvent as TuiKeyEvent},
    Component, Event, MockComponent, State, StateValue, AttrValue, Attribute,
};
use tui_textarea::TextArea;

use crate::client::types::{HitlApprovalRequest, HitlDecisionRequest, HitlDecision as ApiHitlDecision};
use crate::message::{AppMsg, NoUserEvent};

/// Review modes for the HITL window
#[derive(Debug, Clone, PartialEq)]
pub enum ReviewMode {
    /// Viewing the request details
    Viewing,
    /// Editing modification content
    Editing,
    /// Confirming decision
    Confirming,
}

/// Local HITL decision type for UI logic
#[derive(Debug, Clone, PartialEq)]
pub enum HitlDecision {
    Approve,
    Reject, 
    Modify,
}

/// HitlReview component using TUIRealm architecture
pub struct HitlReviewRealmComponent {
    /// Current HITL request being reviewed
    current_request: Option<HitlApprovalRequest>,
    /// Text area for review comments/modifications
    textarea: TextArea<'static>,
    /// Current review mode
    mode: ReviewMode,
    /// User's decision
    decision: Option<HitlDecision>,
    /// Whether the review window is visible
    visible: bool,
    /// Whether this component is focused
    focused: bool,
}

impl HitlReviewRealmComponent {
    /// Create new HITL review component
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(
            Block::default()
                .title(" Review Comments / Modifications ")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow))
        );
        textarea.set_style(Style::default().fg(Color::White));
        textarea.set_placeholder_text("Enter your comments or modifications here...");
        
        Self {
            current_request: None,
            textarea,
            mode: ReviewMode::Viewing,
            decision: None,
            visible: false,
            focused: false,
        }
    }
    
    /// Show review window with a request
    pub fn show_review(&mut self, request: HitlApprovalRequest) {
        self.current_request = Some(request);
        self.mode = ReviewMode::Viewing;
        self.decision = None;
        self.textarea = TextArea::default();
        self.textarea.set_placeholder_text("Enter your comments or modifications here...");
        self.visible = true;
        self.focused = true;
    }
    
    /// Hide the review window
    pub fn hide(&mut self) {
        self.visible = false;
        self.focused = false;
        self.current_request = None;
        self.decision = None;
        self.mode = ReviewMode::Viewing;
    }
    
    /// Set decision
    pub fn set_decision(&mut self, decision: HitlDecision) {
        if decision == HitlDecision::Modify {
            self.mode = ReviewMode::Editing;
        } else {
            self.mode = ReviewMode::Confirming;
        }
        self.decision = Some(decision);
    }
    
    /// Submit the current decision
    pub fn submit_decision(&mut self) -> Option<AppMsg> {
        if let (Some(ref request), Some(decision)) = (&self.current_request, &self.decision) {
            let reason = if self.textarea.lines().iter().any(|line| !line.is_empty()) {
                Some(self.textarea.lines().join("\n"))
            } else {
                None
            };
            
            // Clone the data we need before calling self.hide()
            let request_agent_id = request.agent_id.clone();
            let decision_clone = decision.clone();
            
            // Create the decision request before calling hide()
            let decision_request = HitlDecisionRequest {
                decision: self.convert_to_api_decision(&decision_clone), 
                modified_content: reason.clone(),
                request_id: request_agent_id.clone(),
                reason,
            };
            
            // Now we can call hide() since we've extracted all the data we need
            self.hide();
            
            Some(AppMsg::HitlDecisionMade(
                request_agent_id,
                decision_request
            ))
        } else {
            None
        }
    }
    
    /// Is the component visible?
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    
    /// Convert UI HitlDecision to API HitlDecision
    fn convert_to_api_decision(&self, decision: &HitlDecision) -> ApiHitlDecision {
        match decision {
            HitlDecision::Approve => ApiHitlDecision::Approve,
            HitlDecision::Reject => ApiHitlDecision::Reject,
            HitlDecision::Modify => ApiHitlDecision::Modify,
        }
    }
    
    /// Get the popup area for centering the modal
    fn get_popup_area(&self, area: Rect) -> Rect {
        let popup_width = area.width.min(80);
        let popup_height = area.height.min(30);
        
        Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width,
            height: popup_height,
        }
    }
}

impl Component<AppMsg, NoUserEvent> for HitlReviewRealmComponent {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<AppMsg> {
        if !self.visible || !self.focused {
            return None;
        }
        
        match ev {
            Event::Keyboard(key_event) => {
                match self.mode {
                    ReviewMode::Editing => {
                        // Handle text editing
                        match key_event {
                            TuiKeyEvent { code: Key::Esc, .. } => {
                                self.mode = ReviewMode::Viewing;
                                None
                            },
                            key => {
                                // Convert to crossterm KeyEvent
                                let crossterm_modifiers = KeyModifiers::NONE; // For now, just use NONE
                                let crossterm_event = match key.code {
                                    Key::Char(c) => KeyEvent::new(KeyCode::Char(c), crossterm_modifiers),
                                    Key::Backspace => KeyEvent::new(KeyCode::Backspace, crossterm_modifiers),
                                    Key::Delete => KeyEvent::new(KeyCode::Delete, crossterm_modifiers),
                                    Key::Enter => KeyEvent::new(KeyCode::Enter, crossterm_modifiers),
                                    Key::Left => KeyEvent::new(KeyCode::Left, crossterm_modifiers),
                                    Key::Right => KeyEvent::new(KeyCode::Right, crossterm_modifiers),
                                    Key::Up => KeyEvent::new(KeyCode::Up, crossterm_modifiers),
                                    Key::Down => KeyEvent::new(KeyCode::Down, crossterm_modifiers),
                                    _ => return None,
                                };
                                self.textarea.input(crossterm_event);
                                None
                            }
                        }
                    },
                    ReviewMode::Viewing => {
                        // Handle navigation keys
                        match key_event {
                            TuiKeyEvent { code: Key::Char('a'), .. } => {
                                self.set_decision(HitlDecision::Approve);
                                None
                            },
                            TuiKeyEvent { code: Key::Char('r'), .. } => {
                                self.set_decision(HitlDecision::Reject);
                                None
                            },
                            TuiKeyEvent { code: Key::Char('m'), .. } => {
                                self.set_decision(HitlDecision::Modify);
                                None
                            },
                            TuiKeyEvent { code: Key::Char('q'), .. } | TuiKeyEvent { code: Key::Esc, .. } => {
                                self.hide();
                                None
                            },
                            _ => None,
                        }
                    },
                    ReviewMode::Confirming => {
                        match key_event {
                            TuiKeyEvent { code: Key::Char('y'), .. } | TuiKeyEvent { code: Key::Enter, .. } => {
                                self.submit_decision()
                            },
                            TuiKeyEvent { code: Key::Char('n'), .. } | TuiKeyEvent { code: Key::Esc, .. } => {
                                self.mode = ReviewMode::Viewing;
                                self.decision = None;
                                None
                            },
                            _ => None,
                        }
                    }
                }
            },
            _ => None,
        }
    }
}

impl MockComponent for HitlReviewRealmComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }
        
        let popup_area = self.get_popup_area(area);
        
        // Clear the background
        frame.render_widget(Clear, popup_area);
        
        // Split into sections
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),       // Request details
                Constraint::Length(8),    // Text area (when editing)
                Constraint::Length(3),    // Instructions
            ])
            .split(popup_area);
        
        // Render request details
        if let Some(ref request) = self.current_request {
            let title = format!(" HITL Review - Agent {} ", request.agent_id);
            let border_color = match self.mode {
                ReviewMode::Viewing => Color::Yellow,
                ReviewMode::Editing => Color::Green,
                ReviewMode::Confirming => Color::Red,
            };
            
            let details_text = vec![
                Line::from(""),
                Line::from(vec![Span::styled("Agent:", Style::default().fg(Color::Green))]),
                Line::from(format!("  {} ({:?})", request.agent_id, request.agent_type)),
                Line::from(""),
                Line::from(vec![Span::styled("Context:", Style::default().fg(Color::Magenta))]),
                Line::from(format!("  {}", request.context)),
                Line::from(""),
                Line::from(vec![Span::styled("Proposed Action:", Style::default().fg(Color::Yellow))]),
                Line::from(format!("  {:?}", request.proposed_action)),
                Line::from(""),
                Line::from(vec![Span::styled("Proposed Changes:", Style::default().fg(Color::Cyan))]),
                Line::from(format!("  {} files", request.proposed_changes.len())),
            ];
            
            let details_widget = Paragraph::new(details_text)
                .block(Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)))
                .wrap(Wrap { trim: true });
            
            frame.render_widget(details_widget, chunks[0]);
        }
        
        // Render text area when editing
        if self.mode == ReviewMode::Editing {
            frame.render_widget(&self.textarea, chunks[1]);
        }
        
        // Render instructions
        let instructions = match self.mode {
            ReviewMode::Viewing => {
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("[a]", Style::default().fg(Color::Green)),
                        Span::raw("pprove  "),
                        Span::styled("[r]", Style::default().fg(Color::Red)),
                        Span::raw("eject  "),
                        Span::styled("[m]", Style::default().fg(Color::Yellow)),
                        Span::raw("odify  "),
                        Span::styled("[q]", Style::default().fg(Color::Gray)),
                        Span::raw("uit"),
                    ]),
                ]
            },
            ReviewMode::Editing => {
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Edit your modifications above. Press "),
                        Span::styled("Esc", Style::default().fg(Color::Yellow)),
                        Span::raw(" when done."),
                    ]),
                ]
            },
            ReviewMode::Confirming => {
                vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Confirm decision? "),
                        Span::styled("[y]", Style::default().fg(Color::Green)),
                        Span::raw("es  "),
                        Span::styled("[n]", Style::default().fg(Color::Red)),
                        Span::raw("o"),
                    ]),
                ]
            }
        };
        
        let instructions_widget = Paragraph::new(instructions)
            .block(Block::default().borders(Borders::ALL));
        
        frame.render_widget(instructions_widget, chunks[2]);
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
                    if !visible {
                        self.hide();
                    }
                }
            }
            _ => {}
        }
    }
    
    fn state(&self) -> State {
        if self.visible {
            State::One(StateValue::String(format!("{:?}", self.mode)))
        } else {
            State::None
        }
    }
    
    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Submit => {
                if let Some(_msg) = self.submit_decision() {
                    CmdResult::Submit(State::One(StateValue::String("submitted".to_string())))
                } else {
                    CmdResult::None
                }
            },
            Cmd::Cancel => {
                self.hide();
                CmdResult::Changed(State::None)
            },
            _ => CmdResult::None,
        }
    }
}