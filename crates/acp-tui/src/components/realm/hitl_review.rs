//! HITL Review Modal Component
//!
//! Displays a modal overlay for human-in-the-loop approval requests.
//! Blocks user interaction with other components until a decision is made.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use tui_textarea::{Input, TextArea};
use tui_textarea::Key as TextAreaKey;
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Key, KeyEvent as TuiKeyEvent, KeyModifiers},
    props::{Alignment as PropsAlignment, AttrValue, Attribute, BorderType, Borders as PropBorders},
    Component, Event, MockComponent, State,
};
use tracing::{debug, info, warn};

use crate::{client::{EventType, StatusEvent}, message::{APIEvent, UserEvent}};

/// Input mode for the modal
#[derive(Debug, Clone, PartialEq)]
enum InputMode {
    /// Normal mode - keyboard shortcuts active
    Normal,
    /// Editing reasoning text
    EditingReasoning,
}
/// Risk level for HITL requests
#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    High,    // write_file, bash_command, delete
    Medium,  // install, network calls
    Low,     // read_file, search
}

impl RiskLevel {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            _ => Self::Medium, // Default to medium if unknown
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::High => Color::Red,
            Self::Medium => Color::Yellow,
            Self::Low => Color::Blue,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::High => "🚨",
            Self::Medium => "⚠️ ",
            Self::Low => "ℹ️ ",
        }
    }
}

/// HITL request data
#[derive(Debug, Clone)]
pub struct HitlRequest {
    pub id: String,
    pub tool_name: String,
    pub description: String,
    pub risk_level: RiskLevel,
    pub preview: String,
    pub reasoning: Option<String>,
    pub metadata: HitlMetadata,
}

/// Metadata about the HITL request
#[derive(Debug, Clone)]
pub struct HitlMetadata {
    pub file_path: Option<String>,
    pub file_size: Option<usize>,
    pub is_new_file: bool,
    pub dependencies: Vec<String>,
    pub command: Option<String>,
}

/// HITL Review Modal Component
pub struct HitlReviewRealmComponent {
    /// Current HITL request being displayed
    current_request: Option<HitlRequest>,

    /// Queue of pending requests
    request_queue: Vec<HitlRequest>,

    /// Current scroll position in preview
    scroll_offset: usize,

    /// Whether the modal is visible
    visible: bool,

    /// Auto-approve timer (seconds remaining for low-risk)
    auto_approve_timer: Option<u32>,

    /// Whether timer is paused
    timer_paused: bool,

    /// Current input mode
    input_mode: InputMode,

    /// Textarea for reasoning input
    reasoning_textarea: TextArea<'static>,

    /// Pending decision (approval/rejection waiting for reasoning)
    pending_decision: Option<bool>, // Some(true) = approve, Some(false) = reject, None = no pending
}

impl HitlReviewRealmComponent {
    pub fn new() -> Self {
        let mut reasoning_textarea = TextArea::default();
        reasoning_textarea.set_placeholder_text("Enter your reasoning (optional)...");
        reasoning_textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Reasoning")
                .border_style(Style::default().fg(Color::White)),
        );

        Self {
            current_request: None,
            request_queue: Vec::new(),
            scroll_offset: 0,
            visible: false,
            auto_approve_timer: None,
            timer_paused: false,
            input_mode: InputMode::Normal,
            reasoning_textarea,
            pending_decision: None,
        }
    }

     /// Start editing reasoning
    fn start_reasoning_input(&mut self, approved: bool) {
        self.pending_decision = Some(approved);
        self.input_mode = InputMode::EditingReasoning;
        self.reasoning_textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Reasoning (Ctrl+S to submit, Esc to cancel)")
                .border_style(Style::default().fg(Color::Yellow)),
        );
    }

    /// Cancel reasoning input and return to normal mode
    fn cancel_reasoning_input(&mut self) {
        self.pending_decision = None;
        self.input_mode = InputMode::Normal;
        self.reasoning_textarea = TextArea::default();
        self.reasoning_textarea.set_placeholder_text("Enter your reasoning (optional)...");
        self.reasoning_textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Reasoning")
                .border_style(Style::default().fg(Color::White)),
        );
    }

    /// Submit decision with reasoning
    fn submit_with_reasoning(&mut self) -> Option<UserEvent> {
        if let Some(approved) = self.pending_decision {
            let reasoning_text = self.reasoning_textarea.lines().join("\n");
            let reasoning = if reasoning_text.trim().is_empty() {
                None
            } else {
                Some(reasoning_text)
            };

            info!(
                "HITL decision: {} with reasoning: {:?}",
                if approved { "approved" } else { "rejected" },
                reasoning
            );

            if let Some(request) = &self.current_request {
                let event = UserEvent::HitlDecisionSubmit {
                    id: request.id.clone(),
                    approved,
                    modified_content: None,
                    reasoning
                };

                // Clear textarea and return to normal mode
                self.cancel_reasoning_input();

                // Move to next request
                self.next_request();

                return Some(event);
            }
        }
        None
    }

    /// Handle approval decision (prompts for reasoning)
    fn approve(&mut self) -> Option<UserEvent> {
        self.start_reasoning_input(true);
        None // Don't emit event yet, wait for reasoning
    }

    /// Handle rejection decision (prompts for reasoning)
    fn reject(&mut self) -> Option<UserEvent> {
        self.start_reasoning_input(false);
        None // Don't emit event yet, wait for reasoning
    }

    /// Quick approve without reasoning (Shift+A)
    fn quick_approve(&mut self) -> Option<UserEvent> {
        if let Some(request) = &self.current_request {
            info!("HITL quick approved (no reasoning): {}", request.tool_name);

            let event = UserEvent::HitlDecisionSubmit {
                id: request.id.clone(),
                approved: true,
                modified_content: None,
                reasoning: None
            };

            self.next_request();
            Some(event)
        } else {
            None
        }
    }

    /// Quick reject without reasoning (Shift+R)
    fn quick_reject(&mut self) -> Option<UserEvent> {
        if let Some(request) = &self.current_request {
            info!("HITL quick rejected (no reasoning): {}", request.tool_name);

            let event = UserEvent::HitlDecisionSubmit {
                id: request.id.clone(),
                approved: false,
                modified_content: None,
                reasoning: None,
            };

            self.next_request();
            Some(event)
        } else {
            None
        }
    }

    /// Add a new HITL request to the queue
    pub fn push_request(&mut self, request: HitlRequest) {
        info!(
            "HITL request queued: {} by {}",
            request.tool_name, request.id
        );

        // If no current request, show immediately
        if self.current_request.is_none() {
            self.show_request(request);
        } else {
            self.request_queue.push(request);
        }
    }

    /// Show a request in the modal
    fn show_request(&mut self, request: HitlRequest) {
        // Set auto-approve timer for low-risk operations
        self.auto_approve_timer = if request.risk_level == RiskLevel::Low {
            Some(10) // 10 seconds
        } else {
            None
        };

        self.current_request = Some(request);
        self.visible = true;
        self.scroll_offset = 0;
        self.timer_paused = false;
    }

    /// Move to next request in queue
    fn next_request(&mut self) -> Option<UserEvent> {
        if let Some(next) = self.request_queue.pop() {
            self.show_request(next);
            None
        } else {
            // No more requests, hide modal
            self.current_request = None;
            self.visible = false;
            None
        }
    }

    /// Handle defer decision
    fn defer(&mut self) -> Option<UserEvent> {
        if let Some(request) = self.current_request.take() {
            info!("HITL deferred: {}", request.tool_name);

            // Put at end of queue
            self.request_queue.push(request);

            // Move to next
            self.next_request()
        } else {
            None
        }
    }

    /// Scroll preview up
    fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll preview down
    fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Tick for auto-approve timer
    pub fn tick(&mut self) -> Option<UserEvent> {
        if self.timer_paused {
            return None;
        }

        if let Some(timer) = self.auto_approve_timer.as_mut() {
            if *timer > 0 {
                *timer -= 1;
            } else {
                // Timer expired, auto-approve
                info!("Auto-approving low-risk HITL request");
                return self.approve();
            }
        }
        None
    }

    /// Toggle timer pause
    fn toggle_timer(&mut self) {
        self.timer_paused = !self.timer_paused;
    }

    /// Get current queue position
    fn queue_position(&self) -> String {
        let total = self.request_queue.len() + 1; // +1 for current
        format!("[1/{}]", total)
    }

    /// Check if modal should be visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Render the modal content
    fn render_modal(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(request) = &self.current_request {
            // Center the modal (85% width, 80% height)
            let modal_area = centered_rect(85, 80, area);

            // Clear background
            frame.render_widget(Clear, modal_area);

            // Split into header, body, reasoning, footer
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),   // Header
                    Constraint::Min(8),      // Body (preview)
                    Constraint::Length(5),   // Reasoning textarea
                    Constraint::Length(3),   // Footer
                ])
                .split(modal_area);

            // Render header
            self.render_header(frame, chunks[0], request);

            // Render body
            self.render_body(frame, chunks[1], request);

            // Render reasoning textarea
            frame.render_widget(&self.reasoning_textarea, chunks[2]);

            // Render footer
            self.render_footer(frame, chunks[3], request);
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect, request: &HitlRequest) {
        let risk_color = request.risk_level.color();
        let risk_icon = request.risk_level.icon();

        let title = format!(
            "{} {}                    {}    {}",
            risk_icon,
            request.tool_name,
            request.id,
            self.queue_position()
        );

        let header = Paragraph::new(title)
            .style(Style::default().fg(risk_color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(risk_color))
                    .border_type(ratatui::widgets::BorderType::Double),
            );

        frame.render_widget(header, area);
    }

    fn render_body(&self, frame: &mut Frame, area: Rect, request: &HitlRequest) {
        let mut lines = Vec::new();

        // File/command info
        if let Some(ref path) = request.metadata.file_path {
            let size_info = if let Some(size) = request.metadata.file_size {
                format!(" ({} lines{})", size, if request.metadata.is_new_file { ", NEW" } else { "" })
            } else {
                String::new()
            };

            lines.push(Line::from(vec![
                Span::styled("File: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{}{}", path, size_info)),
            ]));
        }

        if let Some(ref cmd) = request.metadata.command {
            lines.push(Line::from(vec![
                Span::styled("$ ", Style::default().fg(Color::Green)),
                Span::raw(cmd),
            ]));
        }

        // Dependencies
        if !request.metadata.dependencies.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Deps: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(request.metadata.dependencies.join(", ")),
            ]));
        }

        lines.push(Line::from("")); // Blank line

        // Preview box
        lines.push(Line::from("┌────────────────────────────────────────────────────────────┐"));

        let preview_lines: Vec<&str> = request.preview.lines().collect();
        let visible_lines = 10; // Max lines to show
        for (_i, line) in preview_lines.iter()
            .skip(self.scroll_offset)
            .take(visible_lines)
            .enumerate()
        {
            let truncated = if line.len() > 58 {
                format!("{}...", &line[..55])
            } else {
                format!("{:<58}", line)
            };
            lines.push(Line::from(format!("│ {} │", truncated)));
        }

        // Scroll indicator
        let scroll_indicator = if preview_lines.len() > visible_lines {
            format!("[L{}/{:3}] ↓↑", self.scroll_offset + 1, preview_lines.len())
        } else {
            format!("[{} lines]", preview_lines.len())
        };
        lines.push(Line::from(format!("│{:>60}│", scroll_indicator)));
        lines.push(Line::from("└────────────────────────────────────────────────────────────┘"));

        lines.push(Line::from("")); // Blank line

        // Reasoning
        if let Some(ref reasoning) = request.reasoning {
            lines.push(Line::from(vec![
                Span::styled("\"", Style::default().fg(Color::Gray)),
                Span::styled(reasoning, Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)),
                Span::styled("\"", Style::default().fg(Color::Gray)),
            ]));
        }

        // Auto-approve timer
        if let Some(timer) = self.auto_approve_timer {
            lines.push(Line::from(""));
            let timer_text = if self.timer_paused {
                format!("⏸  Auto-approve PAUSED (was {}s)", timer)
            } else {
                format!("⏰ Auto-approving in {}s...", timer)
            };
            lines.push(Line::from(Span::styled(
                timer_text,
                Style::default().fg(Color::Yellow),
            )));
        }

        let body = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: false });

        frame.render_widget(body, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, request: &HitlRequest) {
        let actions = match self.input_mode {
            InputMode::EditingReasoning => {
                if self.pending_decision == Some(true) {
                    "Approving with reason | Ctrl+S: Submit | Esc: Cancel"
                } else {
                    "Rejecting with reason | Ctrl+S: Submit | Esc: Cancel"
                }
            }
            InputMode::Normal => {
                if request.risk_level == RiskLevel::Low {
                    "[A] Approve  [R] Reject  [Space] Pause  [↓↑] Scroll  [Shift+A/R] Quick"
                } else {
                    "[A] Approve  [R] Reject  [D] Defer  [↓↑] Scroll  [Shift+A/R] Quick"
                }
            }
        };

        let style = match self.input_mode {
            InputMode::EditingReasoning => Style::default().fg(Color::Yellow),
            InputMode::Normal => Style::default().fg(Color::White),
        };

        let footer = Paragraph::new(actions)
            .style(style)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(footer, area);
    }
}

impl Component<UserEvent, APIEvent> for HitlReviewRealmComponent {
    fn on(&mut self, ev: Event<APIEvent>) -> Option<UserEvent> {
        // Only handle events when visible
        // if !self.visible {
        //     return None;
        // }

        match ev {
            // Keyboard events - HANDLE BOTH MODES
            Event::Keyboard(keyevent) if self.visible => {
                match self.input_mode {
                    InputMode::EditingReasoning => {
                        // In reasoning mode, pass keys to textarea
                        match keyevent {
                            // Submit reasoning with Ctrl+S
                            TuiKeyEvent {
                                code: Key::Char('s'),
                                modifiers: KeyModifiers::CONTROL,
                            } => self.submit_with_reasoning(),

                            // Cancel with Esc
                            TuiKeyEvent { code: Key::Esc, .. } => {
                                self.cancel_reasoning_input();
                                None
                            }

                            // Pass other keys to textarea
                            _ => {
                                // Convert tuirealm KeyEvent to tui_textarea Input
                                let input = match keyevent.code {
                                    Key::Char(c) => Input {
                                        key: TextAreaKey::Char(c),
                                        ctrl: keyevent.modifiers.contains(KeyModifiers::CONTROL),
                                        alt: keyevent.modifiers.contains(KeyModifiers::ALT),
                                        shift: keyevent.modifiers.contains(KeyModifiers::SHIFT),
                                    },
                                    Key::Backspace => Input {
                                        key: TextAreaKey::Backspace,
                                        ctrl: false,
                                        alt: false,
                                        shift: false,
                                    },
                                    Key::Delete => Input {
                                        key: TextAreaKey::Delete,
                                        ctrl: false,
                                        alt: false,
                                        shift: false,
                                    },
                                    Key::Enter => Input {
                                        key: TextAreaKey::Enter,
                                        ctrl: false,
                                        alt: false,
                                        shift: false,
                                    },
                                    Key::Left => Input {
                                        key: TextAreaKey::Left,
                                        ctrl: false,
                                        alt: false,
                                        shift: false,
                                    },
                                    Key::Right => Input {
                                        key: TextAreaKey::Right,
                                        ctrl: false,
                                        alt: false,
                                        shift: false,
                                    },
                                    Key::Up => Input {
                                        key: TextAreaKey::Up,
                                        ctrl: false,
                                        alt: false,
                                        shift: false,
                                    },
                                    Key::Down => Input {
                                        key: TextAreaKey::Down,
                                        ctrl: false,
                                        alt: false,
                                        shift: false,
                                    },
                                    _ => return None,
                                };

                                self.reasoning_textarea.input(input);
                                None
                            }
                        }
                    }

                    InputMode::Normal => {
                        // Normal mode shortcuts
                        match keyevent {
                            // Approve (prompts for reasoning)
                            TuiKeyEvent { code: Key::Char('a'), modifiers }
                                if !modifiers.contains(KeyModifiers::SHIFT) => {
                                self.approve()
                            }

                            // Quick approve (no reasoning)
                            TuiKeyEvent {
                                code: Key::Char('A'),
                                modifiers: KeyModifiers::SHIFT,
                            } | TuiKeyEvent {
                                code: Key::Char('a'),
                                modifiers: KeyModifiers::SHIFT,
                            } => {
                                self.quick_approve()
                            }

                            // Reject (prompts for reasoning)
                            TuiKeyEvent { code: Key::Char('r'), modifiers }
                                if !modifiers.contains(KeyModifiers::SHIFT) => {
                                self.reject()
                            }

                            // Quick reject (no reasoning)
                            TuiKeyEvent {
                                code: Key::Char('R'),
                                modifiers: KeyModifiers::SHIFT,
                            } | TuiKeyEvent {
                                code: Key::Char('r'),
                                modifiers: KeyModifiers::SHIFT,
                            }  => {
                                self.quick_reject()
                            }

                            TuiKeyEvent { code: Key::Char('d'), .. }
                            | TuiKeyEvent { code: Key::Char('D'), .. } => self.defer(),

                            TuiKeyEvent { code: Key::Char(' '), .. } => {
                                self.toggle_timer();
                                None
                            }

                            TuiKeyEvent { code: Key::Up, .. } => {
                                self.scroll_up();
                                None
                            }

                            TuiKeyEvent { code: Key::Down, .. } => {
                                self.scroll_down();
                                None
                            }

                            TuiKeyEvent { code: Key::Esc, .. } => self.quick_reject(),

                            _ => None,
                        }
                    }
                }
            }
            Event::User(APIEvent::StatusEventReceived(StatusEvent{event: EventType::HitlRequested { task_description, risk_level }, conversation_id, source, timestamp })) => {
                debug!("HITL request received, opening modal");

                // Parse task_description to extract structured data
                // For now, create a basic request - you'll need to enhance this parsing
                let request = HitlRequest {
                    id: conversation_id,  // TODO: Extract from task_description
                    tool_name: "unknown".to_string(), // TODO: Extract from task_description
                    description: task_description.clone(),
                    risk_level: RiskLevel::from_string(&risk_level),
                    preview: task_description.clone(), // TODO: Extract actual preview
                    reasoning: None,
                    metadata: HitlMetadata {
                        file_path: None,
                        file_size: None,
                        is_new_file: false,
                        dependencies: vec![],
                        command: None,
                    },
                };

                self.push_request(request);
                None // Don't propagate, handled internally
            },

                // Handle HITL completion - CLOSE MODAL
            Event::User(APIEvent::StatusEventReceived(StatusEvent{event: EventType::HitlCompleted { approved, reason }, conversation_id, source, timestamp })) => {
                debug!(
                    "HITL completion received: {} (reason: {:?})",
                    if approved { "approved" } else { "rejected" },
                    reason
                );

                if let Some(request) = &self.current_request {
                    // Emit decision event with current request context
                    let event = UserEvent::HitlDecisionSubmit {
                        id: request.id.clone(),
                        approved,
                        modified_content: None,
                        reasoning: request.reasoning.clone(),
                    };

                    // Move to next request or close modal
                    self.next_request();

                    Some(event)
                } else {
                    warn!("Received HitlCompleted but no current request");
                    None
                }
            }
            Event::Tick => {
                // Handle auto-approve timer
                self.tick()
            }

            _ => None,
        }
    }
}

impl MockComponent for HitlReviewRealmComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        if self.visible {
            self.render_modal(frame, area);
        }
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        match attr {
            Attribute::Custom("visible") => Some(AttrValue::Flag(self.visible)),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match attr {
            Attribute::Custom("hitl_request") => {
                // Receive HITL request - this would need custom serialization
                // For now, handled via push_request() method
            }
            _ => {}
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
