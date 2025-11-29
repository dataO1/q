//! Main application component following React/Elm architecture
//!
//! This module implements the main application loop with message-driven state updates,
//! WebSocket integration, and terminal event handling.

use crate::{
    components::{StatusLine, StatusLineMessage, TimelineComponent, TimelineMessage},
    websocket::WebSocketClient,
    models::StatusEvent,
    config::Config,
    utils,
    error::{Error, Result},
};
use anyhow::Context;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame, Terminal,
};
use std::{
    io::{self, stdout},
    time::{Duration, Instant},
};
use tokio::{
    sync::broadcast,
    time::interval,
};
use tracing::{debug, error, info, instrument, warn};

/// Main application state and event loop
pub struct App {
    /// Application configuration
    config: Config,
    
    /// ACP client for API calls
    acp_client: crate::client::AcpClient,
    
    /// Current conversation ID (persistent for session)
    conversation_id: Option<String>,
    
    /// Timeline component
    timeline: TimelineComponent,
    
    /// Status line component
    status_line: StatusLine,
    
    /// Terminal manager
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    
    /// WebSocket client for real-time updates
    websocket_client: Option<WebSocketClient>,
    
    /// Event receiver from WebSocket
    event_receiver: Option<broadcast::Receiver<StatusEvent>>,
    
    /// Current query input
    query_input: String,
    
    /// Input cursor position
    input_cursor: usize,
    
    /// Whether input is focused
    input_focused: bool,
    
    /// Current status message
    status_message: String,
    
    /// Animation tick interval
    animation_interval: tokio::time::Interval,
    
    /// Last animation tick
    last_animation: Instant,
    
    /// App running state
    running: bool,
    
    /// Show help overlay
    show_help: bool,
    
    /// Channel for internal app messages
    internal_sender: tokio::sync::mpsc::UnboundedSender<AppMessage>,
    internal_receiver: tokio::sync::mpsc::UnboundedReceiver<AppMessage>,
}

/// Application messages following Elm pattern
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// Timeline-specific message
    Timeline(TimelineMessage),
    
    /// Status line message
    StatusLine(StatusLineMessage),
    
    /// Keyboard input
    KeyPress(KeyEvent),
    
    /// Terminal resize
    Resize(u16, u16),
    
    /// Animation tick
    Tick,
    
    /// Query submitted
    SubmitQuery(String),
    
    /// Query execution started successfully
    ExecutionStarted {
        conversation_id: String,
        query: String,
    },
    
    /// Query execution failed
    ExecutionFailed { error: String },
    
    /// Connection status change
    ConnectionStatus(String),
    
    /// Show help overlay
    ShowHelp,
    
    /// Hide help overlay
    HideHelp,
    
    /// Quit application
    Quit,
}

impl App {
    /// Create new application instance
    #[instrument(skip(config))]
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing ACP TUI application");
        
        // Initialize terminal
        enable_raw_mode().map_err(Error::Io)?;
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend).map_err(Error::Io)?;
        
        // Initialize ACP client
        let acp_client = crate::client::AcpClient::new(&config.server_url)
            .context("Failed to initialize ACP client")?;
        
        // Animation interval
        let animation_interval = interval(Duration::from_millis(100)); // 10 FPS
        
        // Create internal message channel
        let (internal_sender, internal_receiver) = tokio::sync::mpsc::unbounded_channel();
        
        Ok(Self {
            config,
            acp_client,
            conversation_id: None,
            timeline: TimelineComponent::new(),
            status_line: StatusLine::new(),
            terminal,
            websocket_client: None, // Start as None, connect after first query
            event_receiver: None,
            query_input: String::new(),
            input_cursor: 0,
            input_focused: true,
            status_message: "Ready".to_string(),
            animation_interval,
            last_animation: Instant::now(),
            running: true,
            show_help: false,
            internal_sender,
            internal_receiver,
        })
    }
    
    /// Run the main application loop
    #[instrument(skip(self))]
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting application event loop");
        
        loop {
            if !self.running {
                break;
            }
            
            // Handle terminal events
            if let Some(msg) = self.handle_terminal_events().await? {
                self.update(msg)?;
            }
            
            // Handle internal messages from async tasks
            if let Some(msg) = self.handle_internal_messages() {
                self.update(msg)?;
            }
            
            // Handle WebSocket events
            if let Some(msg) = self.handle_websocket_events() {
                self.update(msg)?;
            }
            
            // Handle animation ticks
            if self.animation_interval.tick().await.elapsed() < Duration::from_millis(110) {
                self.update(AppMessage::Tick)?;
            }
            
            // Render
            self.render()?;
            
            // Small delay to prevent busy loop
            tokio::time::sleep(Duration::from_millis(16)).await; // ~60 FPS
        }
        
        info!("Application loop ended");
        Ok(())
    }
    
    /// Handle terminal events (keyboard, resize)
    #[instrument(skip(self))]
    async fn handle_terminal_events(&self) -> Result<Option<AppMessage>> {
        if event::poll(Duration::from_millis(10)).map_err(Error::Io)? {
            match event::read().map_err(Error::Io)? {
                Event::Key(key_event) => {
                    return Ok(Some(AppMessage::KeyPress(key_event)));
                }
                Event::Resize(width, height) => {
                    return Ok(Some(AppMessage::Resize(width, height)));
                }
                _ => {}
            }
        }
        Ok(None)
    }
    
    /// Handle internal messages from async tasks
    fn handle_internal_messages(&mut self) -> Option<AppMessage> {
        match self.internal_receiver.try_recv() {
            Ok(message) => Some(message),
            Err(_) => None,
        }
    }
    
    /// Handle WebSocket events
    fn handle_websocket_events(&mut self) -> Option<AppMessage> {
        if let Some(ref mut receiver) = self.event_receiver {
            match receiver.try_recv() {
                Ok(status_event) => {
                    debug!("Received WebSocket event: {:?}", status_event.event);
                    Some(AppMessage::Timeline(TimelineMessage::StatusEvent(status_event)))
                }
                Err(broadcast::error::TryRecvError::Empty) => None,
                Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                    warn!("WebSocket receiver lagged, skipped {} messages", skipped);
                    None
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    error!("WebSocket receiver closed");
                    self.status_message = "Connection lost".to_string();
                    None
                }
            }
        } else {
            None
        }
    }
    
    /// Update application state with message (pure function)
    fn update(&mut self, message: AppMessage) -> Result<()> {
        match message {
            AppMessage::Timeline(timeline_msg) => {
                self.timeline.update(timeline_msg);
            }
            
            AppMessage::StatusLine(status_msg) => {
                self.status_line.handle_message(status_msg);
            }
            
            AppMessage::KeyPress(key_event) => {
                self.handle_key_event(key_event)?;
            }
            
            AppMessage::Resize(width, height) => {
                debug!("Terminal resized to {}x{}", width, height);
                self.terminal.resize(Rect::new(0, 0, width, height)).map_err(Error::Io)?;
            }
            
            AppMessage::Tick => {
                let now = Instant::now();
                if now.duration_since(self.last_animation) >= Duration::from_millis(100) {
                    self.timeline.update(TimelineMessage::AnimationTick);
                    self.last_animation = now;
                }
            }
            
            AppMessage::SubmitQuery(query) => {
                self.submit_query(query)?;
            }
            
            AppMessage::ExecutionStarted { conversation_id, query } => {
                info!("Execution started: {} for query: {}", conversation_id, query);
                self.conversation_id = Some(conversation_id.clone());
                self.status_message = format!("🚀 Execution started: {}", query);
                
                // Clear any previous errors and show success
                self.status_line.handle_message(StatusLineMessage::Info(format!("Query executed: {}", query)));
                
                // Start WebSocket connection now that we have conversation_id
                let ws_url = self.acp_client.get_websocket_url(&conversation_id);
                let (mut websocket_client, event_receiver) = WebSocketClient::new(ws_url);
                
                // Store the event receiver immediately
                self.event_receiver = Some(event_receiver);
                
                // Start the WebSocket client asynchronously
                let internal_sender = self.internal_sender.clone();
                let conv_id = conversation_id.clone();
                
                tokio::spawn(async move {
                    match websocket_client.start().await {
                        Ok(()) => {
                            info!("WebSocket connection started for conversation: {}", conv_id);
                            let _ = internal_sender.send(AppMessage::ConnectionStatus(
                                "🔗 Connected to execution stream".to_string()
                            ));
                        }
                        Err(e) => {
                            error!("Failed to start WebSocket connection: {}", e);
                            let _ = internal_sender.send(AppMessage::ConnectionStatus(
                                format!("❌ WebSocket connection failed: {}", e)
                            ));
                        }
                    }
                });
            }
            
            AppMessage::ExecutionFailed { error } => {
                error!("Query execution failed: {}", error);
                self.status_message = format!("❌ Execution failed: {}", error);
                
                // Show error in status line for immediate user visibility
                self.status_line.handle_message(StatusLineMessage::Error(format!("Execution failed: {}", error)));
            }
            
            AppMessage::ConnectionStatus(status) => {
                self.status_message = status;
            }
            
            AppMessage::ShowHelp => {
                self.show_help = true;
            }
            
            AppMessage::HideHelp => {
                self.show_help = false;
            }
            
            AppMessage::Quit => {
                self.running = false;
            }
        }
        
        Ok(())
    }
    
    /// Handle keyboard events
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        if self.show_help {
            // In help mode, any key closes help
            self.show_help = false;
            return Ok(());
        }
        
        match key_event {
            // Quit application
            KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. } |
            KeyEvent { code: KeyCode::Char('q'), modifiers: KeyModifiers::NONE, .. } => {
                self.running = false;
            }
            
            // Show help
            KeyEvent { code: KeyCode::Char('?'), modifiers: KeyModifiers::NONE, .. } |
            KeyEvent { code: KeyCode::F(1), .. } => {
                self.show_help = true;
            }
            
            // Clear timeline
            KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::NONE, .. } => {
                self.timeline.update(TimelineMessage::Reset);
                self.status_message = "Timeline cleared".to_string();
            }
            
            // Scroll timeline
            KeyEvent { code: KeyCode::Up, .. } => {
                self.timeline.update(TimelineMessage::ScrollUp);
            }
            KeyEvent { code: KeyCode::Down, .. } => {
                self.timeline.update(TimelineMessage::ScrollDown);
            }
            KeyEvent { code: KeyCode::PageUp, .. } => {
                for _ in 0..10 {
                    self.timeline.update(TimelineMessage::ScrollUp);
                }
            }
            KeyEvent { code: KeyCode::PageDown, .. } => {
                for _ in 0..10 {
                    self.timeline.update(TimelineMessage::ScrollDown);
                }
            }
            
            // Input handling when focused
            KeyEvent { code: KeyCode::Enter, .. } if self.input_focused => {
                if !self.query_input.trim().is_empty() {
                    let query = self.query_input.clone();
                    self.query_input.clear();
                    self.input_cursor = 0;
                    self.submit_query(query)?;
                }
            }
            
            KeyEvent { code: KeyCode::Char(c), modifiers: KeyModifiers::NONE, .. } if self.input_focused => {
                self.query_input.insert(self.input_cursor, c);
                self.input_cursor += 1;
            }
            
            KeyEvent { code: KeyCode::Backspace, .. } if self.input_focused => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.query_input.remove(self.input_cursor);
                }
            }
            
            KeyEvent { code: KeyCode::Delete, .. } if self.input_focused => {
                if self.input_cursor < self.query_input.len() {
                    self.query_input.remove(self.input_cursor);
                }
            }
            
            KeyEvent { code: KeyCode::Left, .. } if self.input_focused => {
                self.input_cursor = self.input_cursor.saturating_sub(1);
            }
            
            KeyEvent { code: KeyCode::Right, .. } if self.input_focused => {
                if self.input_cursor < self.query_input.len() {
                    self.input_cursor += 1;
                }
            }
            
            KeyEvent { code: KeyCode::Home, .. } if self.input_focused => {
                self.input_cursor = 0;
            }
            
            KeyEvent { code: KeyCode::End, .. } if self.input_focused => {
                self.input_cursor = self.query_input.len();
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    /// Submit query to ACP server
    #[instrument(skip(self), fields(query = %query))]
    fn submit_query(&mut self, query: String) -> Result<()> {
        info!("Submitting query: {}", query);
        self.status_message = format!("⏳ Executing: {}", query);
        
        // Detect project scope for context
        let project_scope = match crate::client::detect_project_scope() {
            Ok(scope) => scope,
            Err(e) => {
                warn!("Failed to detect project scope: {}", e);
                // Show warning in status line but don't fail
                self.status_line.handle_message(StatusLineMessage::Warning(
                    format!("Could not detect project context: {}", e)
                ));
                // Create a default project scope
                crate::client::ProjectScope {
                    root: std::env::current_dir()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    current_file: None,
                    language_distribution: std::collections::HashMap::new(),
                }
            }
        };
        
        // Execute the query asynchronously
        let acp_client = self.acp_client.clone();
        let query_clone = query.clone();
        let internal_sender = self.internal_sender.clone();
        
        tokio::spawn(async move {
            match acp_client.query(&query_clone, project_scope).await {
                Ok(response) => {
                    info!("Query executed successfully, conversation_id: {}", response.conversation_id);
                    let _ = internal_sender.send(AppMessage::ExecutionStarted {
                        conversation_id: response.conversation_id,
                        query: query_clone,
                    });
                }
                Err(e) => {
                    error!("Failed to execute query: {}", e);
                    let _ = internal_sender.send(AppMessage::ExecutionFailed {
                        error: e.to_string(),
                    });
                }
            }
        });
        
        Ok(())
    }
    
    /// Render the application UI
    fn render(&mut self) -> Result<()> {
        let timeline = &self.timeline;
        let status_line = &self.status_line;
        let query_input = &self.query_input;
        let input_focused = self.input_focused;
        let status_message = &self.status_message;
        let show_help = self.show_help;
        
        self.terminal.draw(|frame| {
            Self::render_frame(frame, timeline, status_line, query_input, input_focused, status_message, show_help);
        }).map_err(Error::Io)?;
        
        Ok(())
    }
    
    /// Render a single frame
    fn render_frame(
        frame: &mut Frame,
        timeline: &TimelineComponent,
        status_line: &StatusLine,
        query_input: &str,
        input_focused: bool,
        status_message: &str,
        show_help: bool,
    ) {
        let area = frame.area();
        
        // Main layout: timeline + input + status line + bottom status
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),      // Timeline
                Constraint::Length(3),   // Input
                Constraint::Length(1),   // Status line (error messages)
                Constraint::Length(1),   // Bottom status (app info)
            ])
            .split(area);
        
        // Render timeline
        timeline.render(frame, chunks[0]);
        
        // Render input box
        Self::render_input(frame, chunks[1], query_input, input_focused);
        
        // Render status line for errors/warnings
        status_line.render(frame, chunks[2]);
        
        // Render bottom status line for app info
        Self::render_status(frame, chunks[3], status_message, timeline);
        
        // Render help overlay if shown
        if show_help {
            Self::render_help(frame, area);
        }
    }
    
    /// Render input box
    fn render_input(frame: &mut Frame, area: Rect, query_input: &str, input_focused: bool) {
        let input_text = if input_focused {
            format!("{}│", query_input)
        } else {
            query_input.to_string()
        };
        
        let input = Paragraph::new(input_text)
            .block(Block::default()
                .title(" Query ")
                .borders(Borders::ALL)
                .border_style(if input_focused {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                }));
        
        frame.render_widget(input, area);
    }
    
    /// Render status line
    fn render_status(frame: &mut Frame, area: Rect, status_message: &str, timeline: &TimelineComponent) {
        let (scroll_offset, total_lines) = timeline.scroll_info();
        
        let status_text = format!(
            " {} | Scroll: {}/{} | Press ? for help, q to quit ",
            status_message,
            scroll_offset,
            total_lines
        );
        
        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Gray));
        
        frame.render_widget(status, area);
    }
    
    /// Render help overlay
    fn render_help(frame: &mut Frame, area: Rect) {
        // Semi-transparent background
        let popup_area = Rect {
            x: area.width / 4,
            y: area.height / 4,
            width: area.width / 2,
            height: area.height / 2,
        };
        
        frame.render_widget(Clear, popup_area);
        
        let help_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" ACP TUI Help ", Style::default().fg(Color::Green))
            ]),
            Line::from(""),
            Line::from(" Enter: Submit query"),
            Line::from(" ↑/↓: Scroll timeline"),
            Line::from(" PgUp/PgDn: Scroll faster"),
            Line::from(" c: Clear timeline"),
            Line::from(" ?: Show this help"),
            Line::from(" q: Quit application"),
            Line::from(" Ctrl+C: Force quit"),
            Line::from(""),
            Line::from(" Press any key to close"),
            Line::from(""),
        ];
        
        let help = Paragraph::new(help_text)
            .block(Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)));
        
        frame.render_widget(help, popup_area);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Clean up terminal state
        let _ = disable_raw_mode();
        
        // Stop WebSocket client
        if let Some(mut client) = self.websocket_client.take() {
            tokio::spawn(async move {
                let _ = client.stop().await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_app_creation() {
        let config = Config::default();
        
        // This test might fail in CI without a terminal
        // let app = App::new(config).await;
        // assert!(app.is_ok());
    }
    
    #[test]
    fn test_app_message_handling() {
        // Test message creation
        let msg = AppMessage::Quit;
        matches!(msg, AppMessage::Quit);
    }
}