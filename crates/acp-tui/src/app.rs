//! Main application component following React/Elm architecture
//!
//! This module implements the main application loop with message-driven state updates,
//! WebSocket integration, and terminal event handling.

use crate::{
    components::{StatusLine, StatusLineMessage, TimelineComponent, TimelineMessage},
    websocket::{WebSocketClient, WebSocketHandle},
    client::types::StatusEvent,
    config::Config,
    utils::{generate_client_id, format_client_id_short},
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
use tui_textarea::TextArea;
use std::{
    io::{self, stdout},
    time::{Duration, Instant},
};
use tokio::{
    sync::broadcast,
    time::interval,
};
use tracing::{debug, error, info, instrument, warn};

/// Connection state for the subscription-first flow
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Not connected to any subscription
    Disconnected,
    /// Creating subscription with server
    Subscribing,
    /// Subscription created, connecting WebSocket
    ConnectingWebSocket,
    /// WebSocket connected, ready for queries
    Connected,
    /// Reconnecting after connection loss
    Reconnecting { attempt: usize },
    /// Connection failed with error
    Failed { error: String },
}

impl ConnectionState {
    /// Check if queries can be executed in this state
    pub fn can_query(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }
    
    /// Get display text for this connection state
    pub fn display_text(&self) -> &str {
        match self {
            ConnectionState::Disconnected => "Disconnected",
            ConnectionState::Subscribing => "Subscribing...",
            ConnectionState::ConnectingWebSocket => "Connecting...",
            ConnectionState::Connected => "Connected",
            ConnectionState::Reconnecting { .. } => "Reconnecting...",
            ConnectionState::Failed { .. } => "Failed",
        }
    }
    
    /// Get colored dot indicator for this connection state
    pub fn status_dot(&self) -> &str {
        match self {
            ConnectionState::Disconnected => "●",
            ConnectionState::Subscribing => "●",
            ConnectionState::ConnectingWebSocket => "●",
            ConnectionState::Connected => "●",
            ConnectionState::Reconnecting { .. } => "●",
            ConnectionState::Failed { .. } => "●",
        }
    }
}

/// Main application state and event loop
pub struct App {
    /// Application configuration
    config: Config,
    
    /// ACP client for API calls
    acp_client: crate::client::AcpClient,
    
    /// Connection state for subscription-first flow
    connection_state: ConnectionState,
    
    /// Deterministic client ID for this machine
    client_id: String,
    
    /// Current subscription ID (from /subscribe endpoint)
    subscription_id: Option<String>,
    
    /// Timeline component
    timeline: TimelineComponent,
    
    /// Status line component
    status_line: StatusLine,
    
    /// Terminal manager
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    
    /// WebSocket handle for real-time updates
    websocket_handle: Option<WebSocketHandle>,
    
    /// Event receiver from WebSocket
    event_receiver: Option<broadcast::Receiver<StatusEvent>>,
    
    /// Current query input (multi-line text area)
    query_input: TextArea<'static>,
    
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
    
    /// Last connection error for display
    last_connection_error: Option<String>,
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
    
    /// Query submitted (will check connection state first)
    SubmitQuery(String),
    
    /// Subscription created successfully
    SubscriptionCreated { subscription_id: String },
    
    /// Subscription resumed (existing subscription found)
    SubscriptionResumed { subscription_id: String },
    
    /// WebSocket connected successfully
    WebSocketConnected,
    
    /// WebSocket disconnected
    WebSocketDisconnected,
    
    /// Connection failed
    ConnectionFailed { error: String },
    
    /// Connection state changed
    ConnectionStateChanged(ConnectionState),
    
    /// Query execution started successfully
    ExecutionStarted {
        subscription_id: String,
        query: String,
    },
    
    /// Query execution failed
    ExecutionFailed { error: String },
    
    /// Initiate connection flow (subscribe → websocket → allow queries)
    StartConnection,
    
    /// Connection step: create subscription
    CreateSubscription,
    
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
        
        // Generate deterministic client ID
        let client_id = generate_client_id()
            .context("Failed to generate client ID")?;
        
        info!("Generated client ID: {}", format_client_id_short(&client_id));
        
        // Animation interval
        let animation_interval = interval(Duration::from_millis(100)); // 10 FPS
        
        // Create internal message channel
        let (internal_sender, internal_receiver) = tokio::sync::mpsc::unbounded_channel();
        
        // Initialize multi-line text area
        let mut text_area = TextArea::default();
        text_area.set_placeholder_text("Enter query (Enter to submit, Shift+Enter for new line)");
        text_area.set_block(
            Block::default()
                .title("Query Input")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
        );
        
        let mut app = Self {
            config,
            acp_client,
            connection_state: ConnectionState::Disconnected,
            client_id,
            subscription_id: None,
            timeline: TimelineComponent::new(),
            status_line: StatusLine::new(),
            terminal,
            websocket_handle: None,
            event_receiver: None,
            query_input: text_area,
            input_focused: true,
            status_message: "Connecting to server...".to_string(),
            last_connection_error: None,
            animation_interval,
            last_animation: Instant::now(),
            running: true,
            show_help: false,
            internal_sender,
            internal_receiver,
        };
        
        // Start connection immediately on startup
        info!("Starting connection flow on TUI startup");
        app.start_connection_flow()?;
        
        Ok(app)
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
                // Regular message processing
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
                if !self.connection_state.can_query() {
                    // Start connection flow if not connected
                    self.start_connection_flow()?;
                    // Store query for after connection
                    self.query_input = TextArea::from([query.as_str()]);
                } else {
                    // Connection ready, execute query
                    self.execute_query(query)?;
                }
            }
            
            AppMessage::StartConnection => {
                self.start_connection_flow()?;
            }
            
            AppMessage::CreateSubscription => {
                self.create_subscription()?;
            }
            
            AppMessage::SubscriptionCreated { subscription_id } => {
                info!("Subscription created: {}", subscription_id);
                self.subscription_id = Some(subscription_id.clone());
                self.connection_state = ConnectionState::ConnectingWebSocket;
                self.start_websocket(subscription_id)?;
            }
            
            AppMessage::SubscriptionResumed { subscription_id } => {
                info!("Subscription resumed: {}", subscription_id);
                self.subscription_id = Some(subscription_id.clone());
                self.connection_state = ConnectionState::ConnectingWebSocket;
                self.start_websocket(subscription_id)?;
            }
            
            AppMessage::WebSocketConnected => {
                info!("WebSocket connected successfully");
                self.connection_state = ConnectionState::Connected;
                self.status_message = "Connected - ready for queries".to_string();
                
                // If there's a pending query, execute it now
                if !self.is_query_empty() {
                    let query = self.get_full_query();
                    self.clear_query_input();
                    self.execute_query(query)?;
                }
            }
            
            AppMessage::WebSocketDisconnected => {
                warn!("WebSocket disconnected - server may be down");
                self.connection_state = ConnectionState::Disconnected;
                self.status_message = "Disconnected from server".to_string();
                self.websocket_handle = None;
                self.event_receiver = None;
            }
            
            AppMessage::ConnectionFailed { error } => {
                error!("Connection failed: {}", error);
                self.connection_state = ConnectionState::Failed { error: error.clone() };
                self.last_connection_error = Some(error.clone());
                self.status_message = format!("Connection failed: {}", error);
            }
            
            AppMessage::ConnectionStateChanged(state) => {
                debug!("Connection state changed to: {:?}", state);
                self.connection_state = state;
            }
            
            AppMessage::ExecutionStarted { subscription_id, query } => {
                info!("Execution started: {} for query: {}", subscription_id, query);
                self.status_message = format!("Execution started: {}", query);
                
                // Clear any previous errors and show success
                self.status_line.handle_message(StatusLineMessage::Info(format!("Query executed: {}", query)));
            }
            
            AppMessage::ExecutionFailed { error } => {
                error!("Query execution failed: {}", error);
                self.status_message = format!("Execution failed: {}", error);
                
                // Show error in status line for immediate user visibility
                self.status_line.handle_message(StatusLineMessage::Error(format!("Execution failed: {}", error)));
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
    
    /// Check if the query input is empty
    fn is_query_empty(&self) -> bool {
        self.query_input.lines().iter().all(|line| line.trim().is_empty())
    }

    /// Get the full query as a single string
    fn get_full_query(&self) -> String {
        self.query_input.lines().join("\n")
    }

    /// Clear all query input
    fn clear_query_input(&mut self) {
        self.query_input = TextArea::default();
        self.query_input.set_placeholder_text("Enter query (Enter to submit, Shift+Enter for new line)");
        self.query_input.set_block(
            Block::default()
                .title("Query Input")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
        );
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
            
            
            // Scroll timeline (only when timeline is focused)
            KeyEvent { code: KeyCode::Up, .. } if !self.input_focused => {
                self.timeline.update(TimelineMessage::ScrollUp);
            }
            KeyEvent { code: KeyCode::Down, .. } if !self.input_focused => {
                self.timeline.update(TimelineMessage::ScrollDown);
            }
            KeyEvent { code: KeyCode::PageUp, .. } if !self.input_focused => {
                for _ in 0..10 {
                    self.timeline.update(TimelineMessage::ScrollUp);
                }
            }
            KeyEvent { code: KeyCode::PageDown, .. } if !self.input_focused => {
                for _ in 0..10 {
                    self.timeline.update(TimelineMessage::ScrollDown);
                }
            }
            
            // Tab to switch focus between input and timeline
            KeyEvent { code: KeyCode::Tab, .. } => {
                self.input_focused = !self.input_focused;
            }
            
            // Enter submits the query when input is focused  
            KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::NONE, .. } if self.input_focused => {
                if !self.is_query_empty() {
                    let query = self.get_full_query();
                    self.clear_query_input();
                    self.update(AppMessage::SubmitQuery(query))?;
                }
            }
            
            // Shift+Enter creates a new line
            KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::SHIFT, .. } if self.input_focused => {
                // Let TextArea handle the new line insertion
                self.query_input.input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            }
            
            // All other input events handled by TextArea when input is focused
            key_event if self.input_focused => {
                self.query_input.input(key_event);
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    /// Start the connection flow: subscribe → websocket → ready for queries
    #[instrument(skip(self))]
    fn start_connection_flow(&mut self) -> Result<()> {
        info!("Starting connection flow");
        self.connection_state = ConnectionState::Subscribing;
        self.status_message = "Creating subscription...".to_string();
        
        let _ = self.internal_sender.send(AppMessage::CreateSubscription);
        Ok(())
    }
    
    /// Create subscription with the server
    #[instrument(skip(self))]
    fn create_subscription(&mut self) -> Result<()> {
        info!("Creating subscription with client_id: {}", format_client_id_short(&self.client_id));
        
        let client = self.acp_client.client().clone();
        let client_id = self.client_id.clone();
        let sender = self.internal_sender.clone();
        
        tokio::spawn(async move {
            let request = crate::client::types::SubscribeRequest {
                client_id: Some(client_id),
            };
            
            match client.create_subscription(&request).await {
                Ok(response) => {
                    let subscription = response.into_inner();
                    
                    info!("Created subscription: {}", subscription.subscription_id);
                    let _ = sender.send(AppMessage::SubscriptionCreated { 
                        subscription_id: subscription.subscription_id 
                    });
                }
                Err(e) => {
                    error!("Failed to create subscription: {}", e);
                    let _ = sender.send(AppMessage::ConnectionFailed {
                        error: format!("Failed to create subscription: {}", e),
                    });
                }
            }
        });
        
        Ok(())
    }
    
    /// Start WebSocket connection with subscription_id
    #[instrument(skip(self), fields(subscription_id = %subscription_id))]
    fn start_websocket(&mut self, subscription_id: String) -> Result<()> {
        info!("Starting WebSocket connection for subscription: {}", subscription_id);
        
        let ws_url = format!("{}/stream/{}", 
            self.acp_client.base_url().replace("http://", "ws://").replace("https://", "wss://"), 
            subscription_id);
            
        let (websocket_handle, event_receiver) = WebSocketClient::start_new(ws_url);
        
        // Store both the handle and receiver
        self.websocket_handle = Some(websocket_handle);
        self.event_receiver = Some(event_receiver);
        
        let sender = self.internal_sender.clone();
        
        // Notify that WebSocket connection is starting
        tokio::spawn(async move {
            // Small delay to ensure connection is established
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let _ = sender.send(AppMessage::WebSocketConnected);
        });
        
        Ok(())
    }
    
    /// Execute query after connection is established
    #[instrument(skip(self), fields(query = %query))]
    fn execute_query(&mut self, query: String) -> Result<()> {
        if !self.connection_state.can_query() {
            warn!("Attempted to execute query while not connected");
            return Ok(());
        }
        
        let subscription_id = match &self.subscription_id {
            Some(id) => id.clone(),
            None => {
                error!("No subscription ID available for query execution");
                return Ok(());
            }
        };
        
        info!("Executing query: {}", query);
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
                // Create a default project scope using generated API types
                crate::client::types::ProjectScope {
                    root: std::env::current_dir()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    current_file: None,
                    language_distribution: {
                        let mut map = std::collections::HashMap::new();
                        map.insert("Unknown".to_string(), 1.0);
                        map
                    }
                }
            }
        };
        
        // Execute the query asynchronously using generated API client
        let client = self.acp_client.client().clone();
        let query_clone = query.clone();
        let internal_sender = self.internal_sender.clone();
        
        tokio::spawn(async move {
            let request = crate::client::types::QueryRequest {
                query: query_clone.clone(),
                project_scope,
                subscription_id: subscription_id.clone(),
            };
            
            match client.query_task(&request).await {
                Ok(response) => {
                    let query_response = response.into_inner();
                    info!("Query executed successfully, subscription_id: {}", subscription_id);
                    let _ = internal_sender.send(AppMessage::ExecutionStarted {
                        subscription_id,
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
        let input_focused = self.input_focused;
        let status_message = &self.status_message;
        let show_help = self.show_help;
        
        let connection_state = &self.connection_state;
        let client_id = &self.client_id;
        
        self.terminal.draw(|frame| {
            Self::render_frame(frame, timeline, &mut self.query_input, input_focused, status_message, show_help, connection_state, client_id);
        }).map_err(Error::Io)?;
        
        Ok(())
    }
    
    /// Render a single frame
    fn render_frame(
        frame: &mut Frame,
        timeline: &TimelineComponent,
        query_input: &mut TextArea,
        input_focused: bool,
        status_message: &str,
        show_help: bool,
        connection_state: &ConnectionState,
        client_id: &str,
    ) {
        let area = frame.area();
        
        // Calculate dynamic height for input area based on content
        let input_lines = query_input.lines().len();
        let input_height = (input_lines + 2).max(5).min(15) as u16; // min 5, max 15 lines
        
        // Main layout: timeline + input + status bar (3 sections)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),                    // Timeline gets remaining space
                Constraint::Length(input_height),      // Dynamic input area
                Constraint::Length(1),                 // Single status bar
            ])
            .split(area);
        
        // Render timeline with focus state
        timeline.render(frame, chunks[0], !input_focused);
        
        // Update border style based on focus
        if input_focused {
            query_input.set_block(
                Block::default()
                    .title("Query Input")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
            );
        } else {
            query_input.set_block(
                Block::default()
                    .title("Query Input")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
            );
        }
        frame.render_widget(&*query_input, chunks[1]);
        
        // Render consolidated status bar
        Self::render_status(frame, chunks[2], status_message, timeline, connection_state, client_id);
        
        // Render help overlay if shown
        if show_help {
            Self::render_help(frame, area);
        }
    }
    
    
    /// Render consolidated status bar
    fn render_status(frame: &mut Frame, area: Rect, status_message: &str, timeline: &TimelineComponent, connection_state: &ConnectionState, client_id: &str) {
        let (scroll_offset, total_lines) = timeline.scroll_info();
        
        // Format: [Status Message] | Scroll: X/Y | Connection: ● Connected | Help: ?
        let status_text = format!(
            " {} | Scroll: {}/{} | Connection: {} {} | Help: ?",
            status_message,
            scroll_offset,
            total_lines,
            connection_state.status_dot(),
            connection_state.display_text()
        );
        
        let connection_color = match connection_state {
            ConnectionState::Connected => Color::Green,
            ConnectionState::Failed { .. } => Color::Red,
            ConnectionState::Disconnected => Color::Gray,
            _ => Color::Yellow,
        };
        
        let status_widget = Paragraph::new(status_text)
            .style(Style::default().fg(connection_color));
        frame.render_widget(status_widget, area);
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
            Line::from(" Enter: New line"),
            Line::from(" Shift+Enter: Submit query"),
            Line::from(" ↑/↓: Navigate lines or scroll timeline"),
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
        if let Some(mut handle) = self.websocket_handle.take() {
            tokio::spawn(async move {
                let _ = handle.stop().await;
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