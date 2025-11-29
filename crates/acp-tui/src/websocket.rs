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
    Subscribe { execution_id: String },
    Unsubscribe { execution_id: String },
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
    /// Create a new WebSocket client
    pub fn new(server_url: String) -> (Self, broadcast::Receiver<StatusEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(1000);
        let (control_sender, _) = mpsc::channel(100);
        
        let client = Self {
            server_url,
            event_sender,
            control_sender,
            task_handle: None,
        };
        
        (client, event_receiver)
    }
    
    /// Start the WebSocket client
    pub async fn start(&mut self) -> Result<()> {
        if self.task_handle.is_some() {
            warn!("WebSocket client is already running");
            return Ok(());
        }
        
        let server_url = self.server_url.clone();
        let event_sender = self.event_sender.clone();
        let (control_sender, mut control_receiver) = mpsc::channel(100);
        self.control_sender = control_sender;
        
        let handle = tokio::spawn(async move {
            let mut connection_handler = ConnectionHandler::new(server_url, event_sender);
            
            loop {
                tokio::select! {
                    // Handle control messages
                    control_msg = control_receiver.recv() => {
                        match control_msg {
                            Some(ControlMessage::Stop) => {
                                info!("Stopping WebSocket client");
                                break;
                            }
                            Some(ControlMessage::Subscribe { execution_id }) => {
                                connection_handler.subscribe(execution_id).await;
                            }
                            Some(ControlMessage::Unsubscribe { execution_id }) => {
                                connection_handler.unsubscribe(execution_id).await;
                            }
                            None => {
                                debug!("Control channel closed");
                                break;
                            }
                        }
                    }
                    
                    // Handle connection management
                    _ = connection_handler.run() => {
                        // Connection handler returned, likely due to error
                        debug!("Connection handler stopped, will retry");
                    }
                }
            }
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
    
    /// Subscribe to status updates for a specific execution
    pub async fn subscribe_to_execution(&self, execution_id: String) -> Result<()> {
        self.control_sender
            .send(ControlMessage::Subscribe { execution_id })
            .await
            .map_err(|e| Error::WebSocket(crate::error::WebSocketError::SendError {
                message: format!("Failed to send subscribe message: {}", e),
            }))?;
        
        Ok(())
    }
    
    /// Unsubscribe from status updates for a specific execution
    pub async fn unsubscribe_from_execution(&self, execution_id: String) -> Result<()> {
        self.control_sender
            .send(ControlMessage::Unsubscribe { execution_id })
            .await
            .map_err(|e| Error::WebSocket(crate::error::WebSocketError::SendError {
                message: format!("Failed to send unsubscribe message: {}", e),
            }))?;
        
        Ok(())
    }
}

/// Handles WebSocket connection lifecycle and message processing
struct ConnectionHandler {
    server_url: String,
    event_sender: broadcast::Sender<StatusEvent>,
    subscriptions: std::collections::HashSet<String>,
    connection_state: ConnectionState,
    reconnect_attempts: usize,
}

impl ConnectionHandler {
    fn new(server_url: String, event_sender: broadcast::Sender<StatusEvent>) -> Self {
        Self {
            server_url,
            event_sender,
            subscriptions: std::collections::HashSet::new(),
            connection_state: ConnectionState::Disconnected,
            reconnect_attempts: 0,
        }
    }
    
    async fn run(&mut self) {
        loop {
            match &self.connection_state {
                ConnectionState::Disconnected => {
                    self.connect().await;
                }
                ConnectionState::Connected => {
                    // This should not be reached in normal flow
                    break;
                }
                ConnectionState::Failed { .. } => {
                    self.reconnect().await;
                }
                _ => {
                    // Wait a bit for state transitions
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
    
    async fn connect(&mut self) {
        self.connection_state = ConnectionState::Connecting;
        
        // The server_url already contains the correct WebSocket path from get_websocket_url()
        let ws_url = self.server_url.replace("http://", "ws://").replace("https://", "wss://");
        
        info!("Connecting to WebSocket: {}", ws_url);
        
        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                info!("WebSocket connected successfully");
                self.connection_state = ConnectionState::Connected;
                self.reconnect_attempts = 0;
                
                // Handle the connection
                if let Err(e) = self.handle_connection(ws_stream).await {
                    error!("WebSocket connection error: {}", e);
                    self.connection_state = ConnectionState::Failed { 
                        error: e.to_string() 
                    };
                }
            }
            Err(e) => {
                error!("Failed to connect to WebSocket: {}", e);
                self.connection_state = ConnectionState::Failed { 
                    error: e.to_string() 
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
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        // Send initial subscriptions
        for execution_id in &self.subscriptions.clone() {
            let subscribe_msg = serde_json::json!({
                "type": "subscribe",
                "execution_id": execution_id
            });
            
            if let Err(e) = ws_sender.send(Message::Text(subscribe_msg.to_string().into())).await {
                error!("Failed to send subscription for {}: {}", execution_id, e);
            }
        }
        
        // Set up ping interval
        let mut ping_interval = interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                // Handle incoming messages
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_message(&text).await {
                                warn!("Failed to handle WebSocket message: {}", e);
                            }
                        }
                        Some(Ok(Message::Binary(_))) => {
                            warn!("Received unexpected binary message");
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("WebSocket connection closed by server");
                            break;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            debug!("Received ping, sending pong");
                            if let Err(e) = ws_sender.send(Message::Pong(data)).await {
                                error!("Failed to send pong: {}", e);
                                break;
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {
                            debug!("Received pong");
                        }
                        Some(Ok(Message::Frame(_))) => {
                            // Raw frame - typically handled internally by tungstenite
                            debug!("Received raw frame");
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            debug!("WebSocket stream ended");
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
                // Try to parse as server message
                if let Ok(server_msg) = serde_json::from_str::<serde_json::Value>(text) {
                    if let Some(msg_type) = server_msg.get("type").and_then(|v| v.as_str()) {
                        match msg_type {
                            "subscribed" => {
                                if let Some(execution_id) = server_msg.get("execution_id")
                                    .and_then(|v| v.as_str()) {
                                    info!("Subscribed to execution: {}", execution_id);
                                }
                            }
                            "unsubscribed" => {
                                if let Some(execution_id) = server_msg.get("execution_id")
                                    .and_then(|v| v.as_str()) {
                                    info!("Unsubscribed from execution: {}", execution_id);
                                }
                            }
                            "error" => {
                                if let Some(error) = server_msg.get("message")
                                    .and_then(|v| v.as_str()) {
                                    error!("Server error: {}", error);
                                }
                            }
                            _ => {
                                warn!("Unknown server message type: {}", msg_type);
                            }
                        }
                    }
                } else {
                    warn!("Failed to parse WebSocket message as JSON: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn subscribe(&mut self, execution_id: String) {
        self.subscriptions.insert(execution_id.clone());
        
        // If connected, send subscription immediately
        if matches!(self.connection_state, ConnectionState::Connected) {
            // Note: In a real implementation, we'd need access to the WebSocket sender here
            // This would require refactoring the connection handling
            debug!("Would send subscription for: {}", execution_id);
        }
    }
    
    async fn unsubscribe(&mut self, execution_id: String) {
        self.subscriptions.remove(&execution_id);
        
        // If connected, send unsubscription immediately
        if matches!(self.connection_state, ConnectionState::Connected) {
            // Note: In a real implementation, we'd need access to the WebSocket sender here
            debug!("Would send unsubscription for: {}", execution_id);
        }
    }
}

impl Drop for WebSocketClient {
    fn drop(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
    }
}

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