//! Layout Manager for Auto-Switching Between Timeline and HITL Modes
//!
//! Manages the TUI layout and automatically switches between timeline view
//! and HITL review mode based on pending requests and user interaction.

use crate::components::{
    HitlQueueComponent, HitlQueueMessage, 
    HitlReviewComponent, HitlReviewMessage,
    TimelineComponent, TimelineMessage,
    StatusLine, StatusLineMessage,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tui_textarea::Input;

/// Application layout modes
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutMode {
    /// Timeline-focused view (default)
    Timeline,
    /// HITL-focused view with queue and review
    HitlReview,
    /// Split view showing both
    Split,
}

/// Which panel currently has focus in the layout
#[derive(Debug, Clone, PartialEq)]
pub enum FocusPanel {
    /// Timeline panel has focus
    Timeline,
    /// HITL queue has focus  
    HitlQueue,
    /// HITL review window has focus
    HitlReview,
    /// Status line has focus (for commands)
    StatusLine,
}

/// Layout manager state
pub struct LayoutManager {
    /// Current layout mode
    mode: LayoutMode,
    
    /// Which panel currently has focus
    focus: FocusPanel,
    
    /// Timeline component
    timeline: TimelineComponent,
    
    /// HITL queue component
    hitl_queue: HitlQueueComponent,
    
    /// HITL review component
    hitl_review: HitlReviewComponent,
    
    /// Status line component
    status_line: StatusLine,
    
    /// Whether auto-switching is enabled
    auto_switch: bool,
    
    /// Last known pending request count
    last_pending_count: usize,
}

/// Messages for layout manager updates
#[derive(Debug, Clone)]
pub enum LayoutMessage {
    /// Switch to specific layout mode
    SetMode(LayoutMode),
    
    /// Switch focus to specific panel
    SetFocus(FocusPanel),
    
    /// Toggle auto-switching behavior
    ToggleAutoSwitch,
    
    /// Handle keyboard input for navigation
    Input(Input),
    
    /// Forward message to timeline component
    Timeline(TimelineMessage),
    
    /// Forward message to HITL queue
    HitlQueue(HitlQueueMessage),
    
    /// Forward message to HITL review
    HitlReview(HitlReviewMessage),
    
    /// Forward message to status line
    StatusLine(StatusLineMessage),
    
    /// Auto-switch based on HITL queue state
    AutoSwitch,
}

impl LayoutManager {
    /// Create new layout manager
    pub fn new() -> Self {
        Self {
            mode: LayoutMode::Timeline,
            focus: FocusPanel::Timeline,
            timeline: TimelineComponent::new(),
            hitl_queue: HitlQueueComponent::new(),
            hitl_review: HitlReviewComponent::new(),
            status_line: StatusLine::new(),
            auto_switch: true,
            last_pending_count: 0,
        }
    }
    
    /// Update layout manager with a message
    pub fn update(&mut self, message: LayoutMessage) {
        match message {
            LayoutMessage::SetMode(mode) => {
                self.mode = mode;
                self.adjust_focus_for_mode();
            }
            
            LayoutMessage::SetFocus(focus) => {
                self.focus = focus;
            }
            
            LayoutMessage::ToggleAutoSwitch => {
                self.auto_switch = !self.auto_switch;
            }
            
            LayoutMessage::Input(input) => {
                self.handle_input(input);
            }
            
            LayoutMessage::Timeline(msg) => {
                self.timeline.update(msg);
            }
            
            LayoutMessage::HitlQueue(msg) => {
                self.hitl_queue.update(msg);
                
                // Check if we should auto-switch after queue updates
                if self.auto_switch {
                    self.check_auto_switch();
                }
            }
            
            LayoutMessage::HitlReview(msg) => {
                // Check if we should auto-switch when review window closes
                let should_auto_switch = matches!(msg, HitlReviewMessage::Hide) && self.auto_switch;
                
                self.hitl_review.update(msg);
                
                if should_auto_switch {
                    self.check_auto_switch();
                }
            }
            
            LayoutMessage::StatusLine(msg) => {
                self.status_line.handle_message(msg);
            }
            
            LayoutMessage::AutoSwitch => {
                if self.auto_switch {
                    self.check_auto_switch();
                }
            }
        }
    }
    
    /// Handle keyboard input based on current focus
    fn handle_input(&mut self, input: Input) {
        match input.key {
            // Global navigation keys - using function key with numbers
            tui_textarea::Key::F(1) => {
                self.mode = LayoutMode::Timeline;
                self.focus = FocusPanel::Timeline;
            }
            tui_textarea::Key::F(2) => {
                self.mode = LayoutMode::HitlReview;
                self.focus = FocusPanel::HitlQueue;
            }
            tui_textarea::Key::F(3) => {
                self.mode = LayoutMode::Split;
                self.adjust_focus_for_mode();
            }
            tui_textarea::Key::F(4) => {
                self.auto_switch = !self.auto_switch;
            }
            
            // Tab to cycle through panels
            tui_textarea::Key::Tab => {
                self.cycle_focus();
            }
            
            // Enter to select/activate
            tui_textarea::Key::Enter => {
                self.handle_enter();
            }
            
            // Forward input to focused component
            _ => {
                match self.focus {
                    FocusPanel::Timeline => {
                        // Timeline doesn't need most input, but could handle expand/collapse
                    }
                    FocusPanel::HitlQueue => {
                        match input.key {
                            tui_textarea::Key::Up => {
                                self.hitl_queue.update(HitlQueueMessage::SelectPrevious);
                            }
                            tui_textarea::Key::Down => {
                                self.hitl_queue.update(HitlQueueMessage::SelectNext);
                            }
                            _ => {}
                        }
                    }
                    FocusPanel::HitlReview => {
                        self.hitl_review.update(HitlReviewMessage::Input(input));
                    }
                    FocusPanel::StatusLine => {
                        // Status line could handle command input
                    }
                }
            }
        }
    }
    
    /// Cycle focus between panels based on current mode
    fn cycle_focus(&mut self) {
        match self.mode {
            LayoutMode::Timeline => {
                self.focus = FocusPanel::Timeline;
            }
            LayoutMode::HitlReview => {
                self.focus = match self.focus {
                    FocusPanel::HitlQueue => FocusPanel::HitlReview,
                    FocusPanel::HitlReview => FocusPanel::HitlQueue,
                    _ => FocusPanel::HitlQueue,
                };
            }
            LayoutMode::Split => {
                self.focus = match self.focus {
                    FocusPanel::Timeline => FocusPanel::HitlQueue,
                    FocusPanel::HitlQueue => FocusPanel::HitlReview,
                    FocusPanel::HitlReview => FocusPanel::Timeline,
                    FocusPanel::StatusLine => FocusPanel::Timeline,
                };
            }
        }
    }
    
    /// Handle Enter key based on current focus
    fn handle_enter(&mut self) {
        match self.focus {
            FocusPanel::HitlQueue => {
                // Open review window for selected request
                if let Some(request) = self.hitl_queue.get_selected_request() {
                    self.hitl_review.update(HitlReviewMessage::ShowReview(request.clone()));
                    self.focus = FocusPanel::HitlReview;
                }
            }
            FocusPanel::Timeline => {
                // Could implement timeline node expansion/collapse
            }
            _ => {}
        }
    }
    
    /// Adjust focus when mode changes
    fn adjust_focus_for_mode(&mut self) {
        match self.mode {
            LayoutMode::Timeline => {
                self.focus = FocusPanel::Timeline;
            }
            LayoutMode::HitlReview => {
                self.focus = if self.hitl_queue.is_empty() {
                    FocusPanel::HitlQueue
                } else {
                    FocusPanel::HitlQueue
                };
            }
            LayoutMode::Split => {
                // Keep current focus if valid, otherwise default to timeline
                if !matches!(self.focus, FocusPanel::Timeline | FocusPanel::HitlQueue | FocusPanel::HitlReview) {
                    self.focus = FocusPanel::Timeline;
                }
            }
        }
    }
    
    /// Check if layout should auto-switch based on HITL queue state
    fn check_auto_switch(&mut self) {
        let current_pending = self.hitl_queue.len();
        
        // Switch to HITL mode when new requests arrive
        if current_pending > self.last_pending_count && current_pending > 0 {
            if matches!(self.mode, LayoutMode::Timeline) {
                self.mode = LayoutMode::HitlReview;
                self.focus = FocusPanel::HitlQueue;
            }
        }
        
        // Switch back to timeline when no pending requests
        if current_pending == 0 && self.last_pending_count > 0 {
            if matches!(self.mode, LayoutMode::HitlReview) && !self.hitl_review.is_visible() {
                self.mode = LayoutMode::Timeline;
                self.focus = FocusPanel::Timeline;
            }
        }
        
        self.last_pending_count = current_pending;
    }
    
    /// Get current layout mode
    pub fn mode(&self) -> &LayoutMode {
        &self.mode
    }
    
    /// Get current focus panel
    pub fn focus(&self) -> &FocusPanel {
        &self.focus
    }
    
    /// Get timeline component
    pub fn timeline(&self) -> &TimelineComponent {
        &self.timeline
    }
    
    /// Get mutable timeline component
    pub fn timeline_mut(&mut self) -> &mut TimelineComponent {
        &mut self.timeline
    }
    
    /// Get HITL queue component
    pub fn hitl_queue(&self) -> &HitlQueueComponent {
        &self.hitl_queue
    }
    
    /// Get mutable HITL queue component
    pub fn hitl_queue_mut(&mut self) -> &mut HitlQueueComponent {
        &mut self.hitl_queue
    }
    
    /// Get HITL review component
    pub fn hitl_review(&self) -> &HitlReviewComponent {
        &self.hitl_review
    }
    
    /// Get mutable HITL review component
    pub fn hitl_review_mut(&mut self) -> &mut HitlReviewComponent {
        &mut self.hitl_review
    }
    
    /// Render the layout
    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        match self.mode {
            LayoutMode::Timeline => self.render_timeline_mode(f, area),
            LayoutMode::HitlReview => self.render_hitl_mode(f, area),
            LayoutMode::Split => self.render_split_mode(f, area),
        }
        
        // Always render HITL review window on top if visible
        if self.hitl_review.is_visible() {
            self.hitl_review.render(f, area);
        }
        
        // Render status line at bottom
        let status_area = self.get_status_area(area);
        self.render_status_line(f, status_area);
    }
    
    /// Render timeline-focused mode
    fn render_timeline_mode(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),      // Timeline
                Constraint::Length(3),    // Status line
            ])
            .split(area);
        
        // Render timeline with full focus
        let focused = matches!(self.focus, FocusPanel::Timeline);
        self.timeline.render(f, chunks[0], focused);
    }
    
    /// Render HITL-focused mode
    fn render_hitl_mode(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),       // HITL queue
                Constraint::Length(3),    // Status line
            ])
            .split(area);
        
        // Render HITL queue with focus indicator
        let queue_focused = matches!(self.focus, FocusPanel::HitlQueue);
        self.hitl_queue.render(f, chunks[0], queue_focused);
    }
    
    /// Render split mode showing both timeline and HITL
    fn render_split_mode(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Timeline
                Constraint::Percentage(40), // HITL queue
            ])
            .split(area);
        
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(3), // Status line
            ])
            .split(chunks[1]);
        
        // Render timeline
        let timeline_focused = matches!(self.focus, FocusPanel::Timeline);
        self.timeline.render(f, chunks[0], timeline_focused);
        
        // Render HITL queue
        let queue_focused = matches!(self.focus, FocusPanel::HitlQueue);
        self.hitl_queue.render(f, main_chunks[0], queue_focused);
    }
    
    /// Get area for status line
    fn get_status_area(&self, area: Rect) -> Rect {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);
        
        chunks[1]
    }
    
    /// Render status line with helpful information
    fn render_status_line(&mut self, f: &mut Frame, area: Rect) {
        let mode_text = format!("Mode: {:?}", self.mode);
        let focus_text = format!("Focus: {:?}", self.focus);
        let auto_switch_text = if self.auto_switch { "Auto: ON" } else { "Auto: OFF" };
        let pending_text = format!("HITL: {}", self.hitl_queue.len());
        
        let help_text = "F1: Timeline | F2: HITL | F3: Split | F4: Auto | Tab: Focus | Enter: Select";
        
        let status_content = vec![
            Line::from(vec![
                Span::styled(mode_text, Style::default().fg(Color::Yellow)),
                Span::raw(" | "),
                Span::styled(focus_text, Style::default().fg(Color::Cyan)),
                Span::raw(" | "),
                Span::styled(auto_switch_text, Style::default().fg(if self.auto_switch { Color::Green } else { Color::Gray })),
                Span::raw(" | "),
                Span::styled(pending_text, Style::default().fg(if self.hitl_queue.len() > 0 { Color::Red } else { Color::Gray })),
            ]),
            Line::from(vec![
                Span::styled(help_text, Style::default().fg(Color::Gray)),
            ]),
        ];
        
        let status_paragraph = Paragraph::new(status_content)
            .block(Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Gray)))
            .wrap(Wrap { trim: false });
        
        f.render_widget(status_paragraph, area);
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new()
    }
}