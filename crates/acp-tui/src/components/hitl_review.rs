//! HITL Review Window Component
//!
//! Provides a review window for HITL approval requests using tui-textarea
//! for a vim-like editing experience.

use crate::client::{HitlApprovalRequest, HitlDecision, HitlDecisionRequest};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use tui_textarea::{TextArea, Input, Key};

/// HITL Review window component state
pub struct HitlReviewComponent {
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
}

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

/// Messages for HITL review updates
#[derive(Debug, Clone)]
pub enum HitlReviewMessage {
    /// Show review window with a request
    ShowReview(HitlApprovalRequest),
    /// Hide the review window
    Hide,
    /// Handle keyboard input
    Input(Input),
    /// Set decision (approve/reject/modify)
    SetDecision(HitlDecision),
    /// Submit the review decision
    Submit,
    /// Cancel the review
    Cancel,
}

impl HitlReviewComponent {
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
        }
    }
    
    /// Update component state with a message
    pub fn update(&mut self, message: HitlReviewMessage) {
        match message {
            HitlReviewMessage::ShowReview(request) => {
                self.current_request = Some(request);
                self.mode = ReviewMode::Viewing;
                self.decision = None;
                self.textarea.delete_line_by_head();
                self.textarea.delete_line_by_end();
                self.visible = true;
            }
            
            HitlReviewMessage::Hide => {
                self.visible = false;
                self.current_request = None;
                self.decision = None;
                self.mode = ReviewMode::Viewing;
            }
            
            HitlReviewMessage::Input(input) => {
                match self.mode {
                    ReviewMode::Editing => {
                        // Handle text editing
                        if input.key == Key::Esc {
                            self.mode = ReviewMode::Viewing;
                        } else {
                            self.textarea.input(input);
                        }
                    }
                    ReviewMode::Viewing => {
                        // Handle navigation keys
                        match input.key {
                            Key::Char('a') => self.set_decision(HitlDecision::Approve),
                            Key::Char('r') => self.set_decision(HitlDecision::Reject),
                            Key::Char('m') => {
                                self.set_decision(HitlDecision::Modify);
                                self.mode = ReviewMode::Editing;
                            }
                            Key::Char('q') | Key::Esc => self.visible = false,
                            _ => {}
                        }
                    }
                    ReviewMode::Confirming => {
                        match input.key {
                            Key::Char('y') | Key::Enter => {
                                // Submit decision
                                self.submit_decision();
                            }
                            Key::Char('n') | Key::Esc => {
                                self.mode = ReviewMode::Viewing;
                                self.decision = None;
                            }
                            _ => {}
                        }
                    }
                }
            }
            
            HitlReviewMessage::SetDecision(decision) => {
                self.set_decision(decision);
            }
            
            HitlReviewMessage::Submit => {
                self.submit_decision();
            }
            
            HitlReviewMessage::Cancel => {
                self.visible = false;
                self.decision = None;
                self.mode = ReviewMode::Viewing;
            }
        }
    }
    
    /// Set the review decision
    fn set_decision(&mut self, decision: HitlDecision) {
        self.decision = Some(decision.clone());
        if decision == HitlDecision::Modify {
            self.mode = ReviewMode::Editing;
        } else {
            self.mode = ReviewMode::Confirming;
        }
    }
    
    /// Submit the decision (placeholder - would integrate with API)
    fn submit_decision(&mut self) {
        if let (Some(_request), Some(decision)) = (&self.current_request, &self.decision) {
            // Create decision request
            let decision_request = HitlDecisionRequest {
                request_id: _request.request_id.clone(),
                decision: decision.clone(),
                modified_content: if decision == &HitlDecision::Modify {
                    Some(self.textarea.lines().join("\n"))
                } else {
                    None
                },
                reason: None,
            };
            
            // TODO: Send decision_request to API
            // For now, just hide the window
            self.visible = false;
            self.current_request = None;
            self.decision = None;
            self.mode = ReviewMode::Viewing;
        }
    }
    
    /// Check if the review window is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    
    /// Get the current request being reviewed
    pub fn current_request(&self) -> Option<&HitlApprovalRequest> {
        self.current_request.as_ref()
    }
    
    /// Render the HITL review window
    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }
        
        // Clear the background
        f.render_widget(Clear, area);
        
        // Create popup layout
        let popup_area = self.create_popup_area(area);
        let popup_block = Block::default()
            .title(" HITL Review ")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Yellow));
        
        f.render_widget(&popup_block, popup_area);
        
        // Layout inside popup
        let inner_area = popup_block.inner(popup_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),  // Request details
                Constraint::Min(5),     // Text area / content
                Constraint::Length(3),  // Instructions/buttons
            ])
            .split(inner_area);
        
        // Render request details
        if let Some(request) = &self.current_request {
            self.render_request_details(f, chunks[0], request);
        }
        
        // Render content area based on mode
        match self.mode {
            ReviewMode::Viewing => {
                self.render_proposed_changes(f, chunks[1]);
            }
            ReviewMode::Editing => {
                f.render_widget(&self.textarea, chunks[1]);
            }
            ReviewMode::Confirming => {
                self.render_confirmation(f, chunks[1]);
            }
        }
        
        // Render instructions
        self.render_instructions(f, chunks[2]);
    }
    
    /// Create popup area (centered, 80% of screen)
    fn create_popup_area(&self, area: Rect) -> Rect {
        let popup_width = (area.width * 80) / 100;
        let popup_height = (area.height * 80) / 100;
        let x = (area.width - popup_width) / 2;
        let y = (area.height - popup_height) / 2;
        
        Rect {
            x: area.x + x,
            y: area.y + y,
            width: popup_width,
            height: popup_height,
        }
    }
    
    /// Render request details section
    fn render_request_details(&self, f: &mut Frame, area: Rect, request: &HitlApprovalRequest) {
        let details = vec![
            Line::from(vec![
                Span::styled("Agent: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{} ({})", request.agent_type, request.agent_id)),
            ]),
            Line::from(vec![
                Span::styled("Task: ", Style::default().fg(Color::Cyan)),
                Span::raw(request.task_id.clone()),
            ]),
            Line::from(vec![
                Span::styled("Risk: ", Style::default().fg(Color::Red)),
                Span::styled(
                    request.risk_level.clone(),
                    Style::default()
                        .fg(match request.risk_level.as_str() {
                            "HIGH" => Color::Red,
                            "MEDIUM" => Color::Yellow,
                            _ => Color::Green,
                        })
                        .add_modifier(Modifier::BOLD)
                ),
            ]),
            Line::from(vec![
                Span::styled("Action: ", Style::default().fg(Color::Cyan)),
                Span::raw(request.proposed_action.clone()),
            ]),
            Line::from(vec![
                Span::styled("Context: ", Style::default().fg(Color::Cyan)),
                Span::raw(request.context.clone()),
            ]),
        ];
        
        let paragraph = Paragraph::new(details)
            .block(Block::default()
                .title(" Request Details ")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Green)))
            .wrap(Wrap { trim: false });
        
        f.render_widget(paragraph, area);
    }
    
    /// Render proposed changes
    fn render_proposed_changes(&self, f: &mut Frame, area: Rect) {
        let changes = if let Some(request) = &self.current_request {
            request.proposed_changes
                .iter()
                .enumerate()
                .map(|(i, change)| {
                    Line::from(vec![
                        Span::styled(
                            format!("{}. ", i + 1),
                            Style::default().fg(Color::Yellow)
                        ),
                        Span::styled(
                            format!("{:?}: ", change.change_type),
                            Style::default().fg(Color::Cyan)
                        ),
                        Span::raw(change.content.clone()),
                    ])
                })
                .collect()
        } else {
            vec![]
        };
        
        let paragraph = Paragraph::new(changes)
            .block(Block::default()
                .title(" Proposed Changes ")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Blue)))
            .wrap(Wrap { trim: false });
        
        f.render_widget(paragraph, area);
    }
    
    /// Render confirmation dialog
    fn render_confirmation(&self, f: &mut Frame, area: Rect) {
        let decision_text = if let Some(decision) = &self.decision {
            format!("Confirm decision: {:?}", decision)
        } else {
            "No decision selected".to_string()
        };
        
        let confirmation = vec![
            Line::from(Span::styled(
                decision_text,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            )),
            Line::from(""),
            Line::from("Press 'y' to confirm, 'n' to cancel"),
        ];
        
        let paragraph = Paragraph::new(confirmation)
            .block(Block::default()
                .title(" Confirmation ")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Red)))
            .wrap(Wrap { trim: false });
        
        f.render_widget(paragraph, area);
    }
    
    /// Render instructions
    fn render_instructions(&self, f: &mut Frame, area: Rect) {
        let instructions = match self.mode {
            ReviewMode::Viewing => {
                "Press: 'a' to Approve, 'r' to Reject, 'm' to Modify, 'q' to Quit"
            }
            ReviewMode::Editing => {
                "Edit your modifications. Press 'Esc' when done."
            }
            ReviewMode::Confirming => {
                "Press 'y' to confirm, 'n' to cancel"
            }
        };
        
        let help = Paragraph::new(instructions)
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: false });
        
        f.render_widget(help, area);
    }
}

impl Default for HitlReviewComponent {
    fn default() -> Self {
        Self::new()
    }
}