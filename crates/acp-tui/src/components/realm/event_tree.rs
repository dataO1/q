//! Timeline TUIRealm component
//!
//! Displays execution events in a tree structure with proper TUIRealm integration.
//! Handles StatusEvent updates directly via APIEvent for real-time rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use tuirealm::{
    command::{Cmd, CmdResult, Direction as MoveDirection},
    event::{Key, KeyEvent as TuiKeyEvent},
    Component, Event, MockComponent, State, AttrValue, Attribute,
    props::{PropPayload, PropValue},
};
use tracing::{debug, info, warn};

use crate::{
    client::types::{EventSource, EventType, StatusEvent},
    message::{APIEvent, UserEvent}, models::tree::{TimelineTree, TreeStats},
};

/// Timeline component using proper TUIRealm architecture
pub struct EventTreeRealmComponent {
    /// Timeline tree data structure
    tree: TimelineTree,
    /// Scroll offset for large trees
    scroll_offset: usize,
    /// Whether this component is focused
    focused: bool,
    /// Animation tick counter
    animation_tick: usize,
    /// Maximum number of lines that can be displayed
    max_display_lines: usize,
    /// Cache for rendered lines
    cached_lines: Option<Vec<String>>,
    /// Cache for tree stats
    cached_stats: Option<TreeStats>,
    /// Generation counter to track tree changes
    tree_generation: usize,
}

impl EventTreeRealmComponent {
    /// Create new timeline component
    pub fn new() -> Self {
        Self {
            tree: TimelineTree::new(),
            scroll_offset: 0,
            focused: false,
            animation_tick: 0,
            max_display_lines: 20, // Default, will be updated based on area
            cached_lines: None,
            cached_stats: None,
            tree_generation: 0,
        }
    }

    /// Handle status event and update tree - moved from Application
    pub fn handle_status_event(&mut self, event: StatusEvent) {
        // Invalidate cache since tree will change
        self.invalidate_cache();

        debug!(?event.event, executionid = event.execution_id, "Processing status event in Timeline");

        // Convert StatusEvent to tree operations following the original pattern
        match &event.event {
            EventType::ExecutionStarted { query } => {
                info!(query = query, "Execution started");
                let root = self.tree.add_root(
                    event.execution_id.clone(),
                    format!("Query: {}", query),
                );
                root.start();
            }
            EventType::AgentStarted { context_size } => {
                if let EventSource::Agent { agent_id, agent_type, task_id } = &event.source {
                    info!(
                        agent_id = agent_id,
                        agent_type = ?agent_type,
                        context_size,
                        "Agent started"
                    );
                    self.tree.add_child(
                        event.execution_id.clone(),
                        agent_id.clone(),
                        format!("{:?} Agent (ctx: {})", agent_type, context_size),
                    );
                    if let Some(node) = self.tree.find_node_mut(agent_id) {
                        node.start();
                    }
                }
            }
            EventType::AgentCompleted { result } => {
                if let EventSource::Agent { agent_id, .. } = &event.source {
                    info!(agent_id = agent_id, "Agent completed");
                    if let Some(node) = self.tree.find_node_mut(agent_id) {
                        node.complete();
                    }
                }
            }
            EventType::AgentFailed { error } => {
                if let EventSource::Agent { agent_id, .. } = &event.source {
                    warn!(agent_id = agent_id, error = error, "Agent failed");
                    if let Some(node) = self.tree.find_node_mut(agent_id) {
                        node.fail(Some(error.clone()));
                    }
                }
            }
            EventType::ToolStarted { args } => {
                if let EventSource::Tool { agent_id, tool_name } = &event.source {
                    info!(agent_id = agent_id, tool_name = tool_name, "Tool started");
                    let tool_id = format!("{}:{}", agent_id, tool_name);
                    self.tree.add_child(
                        agent_id.clone(),
                        tool_id.clone(),
                        format!("Tool: {:?} ({})", tool_name, args),
                    );
                    if let Some(node) = self.tree.find_node_mut(&tool_id) {
                        node.start();
                    }
                }
            }
            EventType::ToolCompleted { result } => {
                if let EventSource::Tool { agent_id, tool_name } = &event.source {
                    info!(agent_id = agent_id, tool_name = tool_name, "Tool completed");
                    let tool_id = format!("{}:{}", agent_id, tool_name);
                    if let Some(node) = self.tree.find_node_mut(&tool_id) {
                        node.complete();
                    }
                }
            }
            EventType::ToolFailed { error } => {
                if let EventSource::Tool { agent_id, tool_name } = &event.source {
                    warn!(
                        agent_id = agent_id,
                        tool_name = tool_name,
                        error = error,
                        "Tool failed"
                    );
                    let tool_id = format!("{}:{}", agent_id, tool_name);
                    if let Some(node) = self.tree.find_node_mut(&tool_id) {
                        node.fail(Some(error.clone()));
                    }
                }
            }
            EventType::ExecutionCompleted { result } => {
                info!(executionid = event.execution_id, "Execution completed");
                if let Some(root) = self.tree.find_node_mut(&event.execution_id) {
                    root.complete();
                }
            }
            EventType::ExecutionFailed { error } => {
                warn!(
                    executionid = event.execution_id,
                    error = error,
                    "Execution failed"
                );
                if let Some(root) = self.tree.find_node_mut(&event.execution_id) {
                    root.fail(Some(error.clone()));
                }
            }
            _ => {
                debug!(event_type = ?event.event, "Unhandled event type");
            }
        }
    }

    /// Clear the timeline
    pub fn clear(&mut self) {
        self.tree.clear();
        self.scroll_offset = 0;
        self.animation_tick = 0;
        self.invalidate_cache();
    }

    /// Invalidate the render cache
    fn invalidate_cache(&mut self) {
        self.cached_lines = None;
        self.cached_stats = None;
        self.tree_generation = self.tree_generation.wrapping_add(1);
    }

    /// Scroll up
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll down
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Page up
    pub fn page_up(&mut self) {
        let page_size = self.max_display_lines.max(1);
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    /// Page down
    pub fn page_down(&mut self) {
        let page_size = self.max_display_lines.max(1);
        self.scroll_offset = self.scroll_offset.saturating_add(page_size);
    }

    /// Advance animations only if there are active animations
    pub fn tick(&mut self) {
        if self.has_active_animations() {
            self.animation_tick = self.animation_tick.wrapping_add(1);
            self.tree.advance_animations();
            // Invalidate cache when animations update
            self.invalidate_cache();
        }
    }

    /// Check if there are any active animations
    fn has_active_animations(&self) -> bool {
        // Check if there are any running nodes that would need animation
        let stats = self.tree.get_stats();
        stats.running > 0
    }

    /// Get scroll info
    pub fn scroll_info(&self) -> (usize, usize) {
        let lines = self.tree.render_lines();
        (self.scroll_offset, lines.len())
    }

    /// Apply styling to a line based on status indicators
    fn style_line<'a>(&self, line: &'a str) -> Vec<Span<'a>> {
        let mut spans = Vec::new();

        // Basic line styling based on content
        let style = if line.contains("✗") {
            Style::default().fg(Color::Red)
        } else if line.contains("✓") {
            Style::default().fg(Color::Green)
        } else if line.contains("⟳") {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        spans.push(Span::styled(line, style));
        spans
    }
}

impl Component<UserEvent, APIEvent> for EventTreeRealmComponent {
    fn on(&mut self, ev: Event<APIEvent>) -> Option<UserEvent> {
        match ev {
            Event::Keyboard(keyevent) => {
                if self.focused {
                    match keyevent {
                        TuiKeyEvent { code: Key::Up, .. } => {
                            self.scroll_up();
                            Some(UserEvent::TimelineScrollUp)
                        }
                        TuiKeyEvent { code: Key::Down, .. } => {
                            self.scroll_down();
                            Some(UserEvent::TimelineScrollDown)
                        }
                        TuiKeyEvent { code: Key::PageUp, .. } => {
                            self.page_up();
                            Some(UserEvent::TimelineScrollUp)
                        }
                        TuiKeyEvent { code: Key::PageDown, .. } => {
                            self.page_down();
                            Some(UserEvent::TimelineScrollDown)
                        }
                        TuiKeyEvent { code: Key::Char('c'), .. } => {
                            Some(UserEvent::TimelineClear)
                        }
                        TuiKeyEvent { code: Key::Tab, .. } => Some(UserEvent::FocusNext),
                        TuiKeyEvent { code: Key::Char('q'), .. }
                        | TuiKeyEvent { code: Key::Char('Q'), .. } => Some(UserEvent::Quit),
                        TuiKeyEvent { code: Key::Char('?'), .. } => Some(UserEvent::HelpToggle),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            Event::User(api_event) => {
                match api_event {
                    APIEvent::StatusEventReceived(status_event) => {
                        // 🔥 Handle StatusEvent directly in the component!
                        self.handle_status_event(status_event);
                        None // Don't emit UserEvent - internal state change only
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl MockComponent for EventTreeRealmComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Update max display lines based on area
        self.max_display_lines = area.height.saturating_sub(2) as usize; // Account for borders

        // Use cached lines if available, otherwise render
        let lines = if let Some(ref cached) = self.cached_lines {
            cached.clone()
        } else {
            let rendered = self.tree.render_lines();
            self.cached_lines = Some(rendered.clone());
            rendered
        };

        // Apply scroll offset
        let visible_lines: Vec<Line> = lines
            .iter()
            .skip(self.scroll_offset)
            .take(self.max_display_lines)
            .map(|line| Line::from(self.style_line(line)))
            .collect();

        // Use cached stats if available, otherwise compute
        let stats = if let Some(ref cached) = self.cached_stats {
            cached.clone()
        } else {
            let computed = self.tree.get_stats();
            self.cached_stats = Some(computed.clone());
            computed
        };

        let title = format!(
            " Timeline ↻{} ✓{} ✗{} ⚠{} ",
            stats.running, stats.completed, stats.failed, stats.warnings
        );

        let border_style = if self.focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let paragraph = Paragraph::new(visible_lines)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        match attr {
            Attribute::Focus => Some(AttrValue::Flag(self.focused)),
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
            Attribute::Custom(name) if name == "tick" => {
                self.tick(); // Advance animations
            }
            _ => {}
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Move(MoveDirection::Up) => {
                self.scroll_up();
                CmdResult::Changed(State::None)
            }
            Cmd::Move(MoveDirection::Down) => {
                self.scroll_down();
                CmdResult::Changed(State::None)
            }
            Cmd::Scroll(MoveDirection::Up) => {
                self.page_up();
                CmdResult::Changed(State::None)
            }
            Cmd::Scroll(MoveDirection::Down) => {
                self.page_down();
                CmdResult::Changed(State::None)
            }
            _ => CmdResult::None,
        }
    }
}
