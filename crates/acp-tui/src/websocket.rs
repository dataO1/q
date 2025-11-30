//! WebSocket client for receiving real-time status updates from ACP server
//!
//! This module handles the WebSocket connection to the ACP server for receiving
//! real-time status updates during query execution. It manages connection lifecycle,
//! message parsing, and event forwarding to the application state.

use crate::{models::StatusEvent, error::{Error, Result}};
use futures_util::{SinkExt, StreamExt};
use serde_json;
use std::time::Duration;
use tokio::{
    sync::{broadcast, mpsc},
    time::{interval, timeout},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tracing::{debug, error, info, warn};

/// Handle for controlling a running WebSocket client
pub struct WebSocketHandle {
    /// Control channel for stopping the client
    control_sender: mpsc::Sender<ControlMessage>,
    
    /// Task handle for the WebSocket connection
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl WebSocketHandle {
    /// Stop the WebSocket client
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            // Send stop signal
            if self.control_sender.send(ControlMessage::Stop).await.is_err() {
                warn!("Failed to send stop signal to WebSocket client");
            }
            
            // Wait for the task to complete with timeout
            match timeout(Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {
                    info!("WebSocket client stopped successfully");
                }
                Ok(Err(e)) => {
                    error!("WebSocket client task panicked: {}", e);
                }
                Err(_) => {
                    warn!("WebSocket client stop timed out");
                }
            }
        }
        
        Ok(())
    }
}

/// WebSocket client for ACP server status updates
pub struct WebSocketClient {
    /// Server URL for WebSocket connection
    server_url: String,
    
    /// Sender for status events
    event_sender: broadcast::Sender<StatusEvent>,
    
    /// Control channel for stopping the client
    control_sender: mpsc::Sender<ControlMessage>,
    
    /// Task handle for the WebSocket connection
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Control messages for WebSocket client
#[derive(Debug)]
enum ControlMessage {
    Stop,
}

/// WebSocket connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: usize },
    Failed { error: String },
}

impl WebSocketClient {
    /// Create a new WebSocket client (URL will be set when starting with subscription_id)
    pub fn new() -> (Self, broadcast::Receiver<StatusEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(1000);
        let (control_sender, _) = mpsc::channel(100);
        
        let client = Self {
            server_url: String::new(), // Will be set in start()
            event_sender,
            control_sender,
            task_handle: None,
        };
        
        (client, event_receiver)
    }
    
    /// Create and start a new WebSocket client, returning handle for control
    pub fn start_new(websocket_url: String) -> (WebSocketHandle, broadcast::Receiver<StatusEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(1000);
        let (control_sender, mut control_receiver) = mpsc::channel(100);
        
        let server_url = websocket_url.clone();
        let control_sender_clone = control_sender.clone();
        
        let handle = tokio::spawn(async move {
            info!("WebSocket client task starting");
            let mut connection_handler = ConnectionHandler::new(server_url.clone(), event_sender);
            
            // Spawn the connection handler in its own task
            let mut connection_task = tokio::spawn(async move {
                info!("Spawning ConnectionHandler task");
                connection_handler.run().await;
                info!("ConnectionHandler task ended");
            });
            
            // Handle control messages in this task
            loop {
                tokio::select! {
                    // Handle control messages
                    control_msg = control_receiver.recv() => {
                        match control_msg {
                            Some(ControlMessage::Stop) => {
                                info!("Received stop signal for WebSocket client");
                                connection_task.abort();
                                break;
                            }
                            None => {
                                warn!("Control channel closed for WebSocket client");
                                break;
                            }
                        }
                    }
                    
                    // Check if connection task ended unexpectedly
                    _ = &mut connection_task => {
                        warn!("Connection handler task ended unexpectedly");
                        break;
                    }
                }
            }
            
            info!("WebSocket client task ending");
        });
        
        let ws_handle = WebSocketHandle {
            control_sender: control_sender_clone,
            task_handle: Some(handle),
        };
        
        (ws_handle, event_receiver)
    }
    
    /// Start the WebSocket client with full WebSocket URL
    pub async fn start(&mut self, websocket_url: String) -> Result<()> {
        if self.task_handle.is_some() {
            warn!("WebSocket client is already running");
            return Ok(());
        }
        
        // Store the full WebSocket URL
        self.server_url = websocket_url;
        
        let server_url = self.server_url.clone();
        let event_sender = self.event_sender.clone();
        let (control_sender, mut control_receiver) = mpsc::channel(100);
        self.control_sender = control_sender;
        
        let handle = tokio::spawn(async move {
            info!("WebSocket client task starting");
            let mut connection_handler = ConnectionHandler::new(server_url.clone(), event_sender);
            
            // Spawn the connection handler in its own task
            let mut connection_task = tokio::spawn(async move {
                info!("Spawning ConnectionHandler task");
                connection_handler.run().await;
                info!("ConnectionHandler task ended");
            });
            
            // Handle control messages in this task
            loop {
                tokio::select! {
                    // Handle control messages
                    control_msg = control_receiver.recv() => {
                        match control_msg {
                            Some(ControlMessage::Stop) => {
                                info!("Received stop signal for WebSocket client");
                                connection_task.abort();
                                break;
                            }
                            None => {
                                debug!("Control channel closed, stopping WebSocket client");
                                connection_task.abort();
                                break;
                            }
                        }
                    }
                    
                    // Check if connection task ended unexpectedly
                    _ = &mut connection_task => {
                        warn!("Connection handler task ended unexpectedly");
                        break;
                    }
                }
            }
            
            info!("WebSocket client task ending");
        });
        
        self.task_handle = Some(handle);
        Ok(())
    }
    
    /// Stop the WebSocket client
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.task_handle.take() {
            // Send stop signal
            if self.control_sender.send(ControlMessage::Stop).await.is_err() {
                warn!("Failed to send stop signal to WebSocket client");
            }
            
            // Wait for the task to complete with timeout
            match timeout(Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {
                    info!("WebSocket client stopped successfully");
                }
                Ok(Err(e)) => {
                    error!("WebSocket client task panicked: {}", e);
                }
                Err(_) => {
                    warn!("WebSocket client stop timed out");
                }
            }
        }
        
        Ok(())
    }
    
    // Note: No subscription methods needed since server streams all events for the conversation_id
}

/// Handles WebSocket connection lifecycle and message processing
struct ConnectionHandler {
    server_url: String,
    event_sender: broadcast::Sender<StatusEvent>,
    connection_state: ConnectionState,
    reconnect_attempts: usize,
}

impl ConnectionHandler {
    fn new(server_url: String, event_sender: broadcast::Sender<StatusEvent>) -> Self {
        Self {
            server_url,
            event_sender,
            connection_state: ConnectionState::Disconnected,
            reconnect_attempts: 0,
        }
    }
    
    async fn run(&mut self) {
        debug!("ConnectionHandler run loop starting");
        loop {
            debug!("Connection state: {:?}", self.connection_state);
            match &self.connection_state {
                ConnectionState::Disconnected => {
                    info!("Attempting to connect to WebSocket");
                    self.connect().await;
                    // connect() will set state to Connected or Failed
                    // If Connected, handle_connection() runs until disconnection
                    // When it returns, connect() sets state to Failed
                }
                ConnectionState::Failed { error } => {
                    warn!("Connection failed: {}. Will retry.", error);
                    self.reconnect().await;
                    // reconnect() will set state back to Disconnected after delay
                }
                ConnectionState::Connecting => {
                    // Should not stay in this state - it's transient
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                ConnectionState::Reconnecting { .. } => {
                    // Should not stay in this state - it's transient  
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                ConnectionState::Connected => {
                    // This should not happen in normal flow
                    // If we reach here, something is wrong with state management
                    error!("Invalid state: Connected in run loop - resetting to Disconnected");
                    self.connection_state = ConnectionState::Failed { 
                        error: "Invalid state transition".to_string() 
                    };
                }
            }
        }
    }
    
    async fn connect(&mut self) {
        debug!("Setting state to Connecting");
        self.connection_state = ConnectionState::Connecting;
        
        // The server_url already contains the correct WebSocket path from get_websocket_url()
        let ws_url = self.server_url.replace("http://", "ws://").replace("https://", "wss://");
        
        info!("Connecting to WebSocket: {}", ws_url);
        
        match connect_async(&ws_url).await {
            Ok((ws_stream, response)) => {
                info!("WebSocket connected successfully. Response status: {:?}", response.status());
                debug!("Setting state to Connected");
                self.connection_state = ConnectionState::Connected;
                self.reconnect_attempts = 0;
                
                // Handle the connection - this will block until connection closes
                info!("Starting connection handler loop");
                if let Err(e) = self.handle_connection(ws_stream).await {
                    error!("WebSocket connection handler error: {}", e);
                    self.connection_state = ConnectionState::Failed { 
                        error: format!("Connection handler error: {}", e) 
                    };
                } else {
                    info!("WebSocket connection closed gracefully");
                    self.connection_state = ConnectionState::Failed { 
                        error: "Connection closed".to_string() 
                    };
                }
            }
            Err(e) => {
                error!("Failed to establish WebSocket connection to {}: {}", ws_url, e);
                self.connection_state = ConnectionState::Failed { 
                    error: format!("Connection failed: {}", e)
                };
            }
        }
    }
    
    async fn reconnect(&mut self) {
        self.reconnect_attempts += 1;
        self.connection_state = ConnectionState::Reconnecting { 
            attempt: self.reconnect_attempts 
        };
        
        // Exponential backoff with max delay
        let delay = std::cmp::min(1000 * (1 << self.reconnect_attempts), 30000);
        warn!(
            "Reconnecting to WebSocket (attempt {}) in {}ms", 
            self.reconnect_attempts, 
            delay
        );
        
        tokio::time::sleep(Duration::from_millis(delay)).await;
        
        // Reset to disconnected to trigger reconnection
        self.connection_state = ConnectionState::Disconnected;
    }
    
    async fn handle_connection(
        &mut self, 
        ws_stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>
    ) -> Result<()> {
        info!("WebSocket connection handler starting");
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        // Server is unidirectional - it only sends status events
        // No subscription messages needed since the server streams all events for this conversation_id
        
        // Set up ping interval
        let mut ping_interval = interval(Duration::from_secs(30));
        
        info!("WebSocket connection handler entering message loop");
        loop {
            tokio::select! {
                // Handle incoming messages
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            info!("Received WebSocket text message: {} chars", text.len());
                            if let Err(e) = self.handle_message(&text).await {
                                warn!("Failed to handle WebSocket message: {}", e);
                            }
                        }
                        Some(Ok(Message::Binary(data))) => {
                            warn!("Received unexpected binary message: {} bytes", data.len());
                        }
                        Some(Ok(Message::Close(close_frame))) => {
                            info!("WebSocket connection closed by server: {:?}", close_frame);
                            break;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            debug!("Received ping ({} bytes), sending pong", data.len());
                            if let Err(e) = ws_sender.send(Message::Pong(data)).await {
                                error!("Failed to send pong: {}", e);
                                break;
                            }
                        }
                        Some(Ok(Message::Pong(data))) => {
                            debug!("Received pong ({} bytes)", data.len());
                        }
                        Some(Ok(Message::Frame(_))) => {
                            // Raw frame - typically handled internally by tungstenite
                            debug!("Received raw frame");
                        }
                        Some(Err(e)) => {
                            error!("WebSocket receive error: {}", e);
                            break;
                        }
                        None => {
                            info!("WebSocket stream ended (receiver returned None)");
                            break;
                        }
                    }
                }
                
                // Send periodic pings
                _ = ping_interval.tick() => {
                    debug!("Sending ping");
                    if let Err(e) = ws_sender.send(Message::Ping(vec![].into())).await {
                        error!("Failed to send ping: {}", e);
                        break;
                    }
                }
            }
        }
        
        info!("WebSocket connection handler exiting");
        Ok(())
    }
    
    async fn handle_message(&mut self, text: &str) -> Result<()> {
        debug!("Received WebSocket message: {}", text);
        
        // Try to parse as status event
        match serde_json::from_str::<StatusEvent>(text) {
            Ok(event) => {
                debug!("Parsed status event: {:?}", event.event);
                
                // Forward event to application
                if let Err(e) = self.event_sender.send(event) {
                    warn!("Failed to forward status event: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to parse WebSocket message as StatusEvent: {}", e);
                debug!("Raw message: {}", text);
            }
        }
        
        Ok(())
    }
    
    // Note: No subscription methods needed - server streams all conversation events automatically
}

// Note: Removed automatic Drop implementation to prevent premature task abortion
// The WebSocket connection should be explicitly stopped via stop() method instead

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_websocket_client_creation() {
        let (client, _receiver) = WebSocketClient::new("ws://localhost:9999".to_string());
        assert_eq!(client.server_url, "ws://localhost:9999");
    }
    
    #[test]
    fn test_connection_state_transitions() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_ne!(ConnectionState::Connecting, ConnectionState::Connected);
    }
}